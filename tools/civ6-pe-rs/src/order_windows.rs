//! Capture-only adapter over the existing observer's attach/cleanup machinery.
use super::*;
use crate::order_capture::{self as plan, Phase, Request, Site};
#[path = "order_context.rs"]
mod context_facts;
use windows_sys::Win32::System::Memory::{
    VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_IMAGE, PAGE_EXECUTE,
    PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_GUARD,
};

const TF: u32 = 1 << 8;
const DEFAULT_CHAIN: &str = "D:/CC/Tools/civ6-live/live_chain.exe";
const AMD_CHAIN: &str = "D:/CC/Tools/civ6-live/live_chain_pr86_amd.exe";
const AMD_CHAIN_SHA: &str = "267ee55fdbe47e507bbefcbb8154780bb2a4070412802f923e0762764d7dc82b";

fn approved_chain_image(path: &Path) -> Result<&'static str, String> {
    [DEFAULT_CHAIN, AMD_CHAIN]
        .into_iter()
        .find(|image| same_path(path, Path::new(image)))
        .ok_or_else(|| "order observer chain image is not approved".into())
}
fn pin_hashed_chain_file(path: &Path, expected_sha: &str) -> Result<File, String> {
    pin_hashed_image_file(path, expected_sha, "AMD chain")
}
fn pin_hashed_image_file(path: &Path, expected_sha: &str, purpose: &str) -> Result<File, String> {
    use std::os::windows::fs::OpenOptionsExt;
    // Keep the approved on-disk image immutable through GO, observation and
    // recovery. Hash once while denying writes/deletes, not on every event.
    let mut pin = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .map_err(|e| format!("{purpose} image pin failed: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = pin
            .read(&mut buffer)
            .map_err(|e| format!("{purpose} image read failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    if format!("{:x}", hasher.finalize()) != expected_sha {
        return Err(format!("order observer {purpose} SHA mismatch"));
    }
    Ok(pin)
}
fn pin_chain_image(path: &Path) -> Result<Option<File>, String> {
    let image = approved_chain_image(path)?;
    if image == DEFAULT_CHAIN {
        return Ok(None);
    }
    pin_hashed_chain_file(Path::new(image), AMD_CHAIN_SHA).map(Some)
}
struct OrderChain {
    process: Handle,
    image: &'static str,
    _image_pin: Option<File>,
}
impl OrderChain {
    fn pin(pid: u32, created: u64) -> Result<Self, String> {
        let process = process(pid, false)?;
        if created == 0 || !alive(&process)? || process_created(&process)? != created {
            return Err("order observer chain PID/FILETIME changed".into());
        }
        let image = approved_chain_image(&process_path(&process)?)?;
        let image_pin = pin_chain_image(Path::new(image))?;
        Ok(Self {
            process,
            image,
            _image_pin: image_pin,
        })
    }
}
fn verify_order_lock(lock: &Path, chain_pid: u32, chain: &OrderChain) -> Result<(), String> {
    verify_lock_for_image(lock, chain_pid, &chain.process, Path::new(chain.image))
}
struct Step {
    site: Site,
    expected: u64,
    armed_eflags: u32,
    capture: Value,
    phase: Phase,
}

fn executable(game: &Handle, base: u64, address: u64, len: usize) -> Result<(), String> {
    let mut mbi = MEMORY_BASIC_INFORMATION::default();
    if unsafe {
        VirtualQueryEx(
            game.0,
            address as *const _,
            &mut mbi,
            size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    } == 0
        || mbi.State != MEM_COMMIT
        || mbi.Type != MEM_IMAGE
        || mbi.AllocationBase as u64 != base
        || mbi.Protect & PAGE_GUARD != 0
        || !matches!(
            mbi.Protect & 0xff,
            PAGE_EXECUTE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY
        )
        || address
            .checked_add(len as u64)
            .is_none_or(|end| end > mbi.BaseAddress as u64 + mbi.RegionSize as u64)
    {
        return Err("site is not inside the expected executable image page".into());
    }
    Ok(())
}
fn instruction(game: &Handle, base: u64, site: Site) -> Result<(), String> {
    let address = base.checked_add(site.rva).ok_or("address overflow")?;
    executable(game, base, address, site.bytes.len())?;
    let mut bytes = vec![0; site.bytes.len()];
    let mut got = 0;
    if unsafe {
        ReadProcessMemory(
            game.0,
            address as *const _,
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            &mut got,
        )
    } == 0
        || got != bytes.len()
        || bytes != site.bytes
    {
        return Err(format!(
            "live opcode signature mismatch at 0x{:x}",
            site.rva
        ));
    }
    Ok(())
}
#[derive(Clone, Copy)]
struct CaptureEpoch {
    phase: Phase,
    generation: u64,
    sites: [Option<u64>; 4],
}
impl CaptureEpoch {
    fn new(
        phase: Phase,
        generation: u64,
        base: u64,
        core: Option<u64>,
        req: &Request,
    ) -> Result<Self, String> {
        let mut sites = [None; 4];
        for (i, site) in plan::prefix(phase, req.count(phase))?.iter().enumerate() {
            sites[i] = Some(
                site.image
                    .base(base, core)?
                    .checked_add(site.rva)
                    .ok_or("capture address overflow")?,
            );
        }
        Ok(Self {
            phase,
            generation,
            sites,
        })
    }

    fn validate_native_image(&self, d: &Debugger) -> Result<(), String> {
        if !self.phase.is_native() {
            return Ok(());
        }
        if d.cleanup_event.is_none()
            && (d.threads.is_empty()
                || d.threads.iter().any(|(tid, state)| {
                    (unsafe { WaitForSingleObject(state.handle.0, 0) }) == WAIT_TIMEOUT
                        && !d.held.contains(tid)
                }))
        {
            return Err(
                "native image validation requires an owned debug stop or complete freeze".into(),
            );
        }
        let core = core_module(d.pid)?.ok_or("stopped native capture Core missing")?;
        if d.order_core_pin.is_none() {
            return Err("native capture lacks retained Core identity".into());
        }
        let core_path = Path::new(GAME_ROOT).join(MODULE_REL);
        let expected_core = self.sites[0]
            .and_then(|address| address.checked_sub(self.phase.sites()[0].rva))
            .ok_or("native capture entry missing")?;
        let expected_exe = self
            .phase
            .sites()
            .iter()
            .enumerate()
            .find(|(_, site)| site.image == plan::Image::Exe)
            .map(|(i, site)| self.sites[i].and_then(|address| address.checked_sub(site.rva)))
            .unwrap_or(Some(0))
            .ok_or("native capture EXE site missing")?;
        // Called only with an outstanding debug event or the complete phase
        // freeze. GO/attach preparation is not a stopped module snapshot.
        if core.base != expected_core || !same_path(&core.path, &core_path) {
            return Err("stopped native capture Core identity differs".into());
        }
        for site in *self.phase.sites() {
            instruction(
                &d.game,
                site.image.base(expected_exe, Some(core.base))?,
                site,
            )?;
        }
        Ok(())
    }
}
#[derive(Default)]
struct CaptureThreads {
    generations: BTreeMap<u32, u64>,
    lifetimes: BTreeMap<u32, u64>,
    created: u64,
    exited: u64,
}
fn arm_evidence(
    d: &Debugger,
    tid: u32,
    from: Option<CaptureEpoch>,
    to: CaptureEpoch,
    generations: &BTreeMap<u32, u64>,
) -> (Value, Result<CONTEXT, String>, u32) {
    let state = &d.threads[&tid];
    let wait = unsafe { WaitForSingleObject(state.handle.0, 0) };
    let mut exit_code = 0;
    let exit_ok = unsafe {
        windows_sys::Win32::System::Threading::GetExitCodeThread(state.handle.0, &mut exit_code)
    } != 0;
    let c = context(&state.handle);
    let mut row = json!({"type":"arm_state","tid":tid,"expected_owner_pid":d.pid,"thread_owner_pid":unsafe{GetProcessIdOfThread(state.handle.0)},"handle_tid":unsafe{GetThreadId(state.handle.0)},"handle_wait":wait,"exit_code":if exit_ok{Some(exit_code)}else{None},"held":d.held.contains(&tid),"armed_sites":state.armed_sites,"expected_phase":from.map(|e|e.phase.name()),"expected_generation":from.map(|e|e.generation),"last_written_generation":generations.get(&tid),"target_phase":to.phase.name(),"target_generation":to.generation,"target_sites":to.sites});
    row["debugger_visibility"] =
        query_thread_visibility(&state.handle, d.query_thread_start).evidence();
    match &c {
        Ok(c) => {
            row["raw_context"] = json!({"context_flags":c.ContextFlags,"rip":c.Rip,"eflags":c.EFlags,"dr0":c.Dr0,"dr1":c.Dr1,"dr2":c.Dr2,"dr3":c.Dr3,"dr6":c.Dr6,"dr7":c.Dr7})
        }
        Err(e) => row["raw_context_error"] = json!(e),
    }
    (row, c, wait)
}
fn reject_arm(d: &mut Debugger, mut row: Value, error: String) -> String {
    row["type"] = json!("arm_rejected");
    row["error"] = json!(error);
    if let Err(write_error) = mark(d, row.clone()) {
        let _ = writeln!(
            std::io::stderr(),
            "{}",
            json!({"type":"arm_record_failure","error":write_error,"evidence":row})
        );
    }
    error
}
// A stop-time sample can prove loss at that instant, never continuous coverage.
// Call only after the existing cleanup freeze, before TF or DR restoration.
fn stop_register_evidence(
    d: &Debugger,
    epoch: CaptureEpoch,
    threads: &CaptureThreads,
    steps: &BTreeMap<u32, Step>,
) -> Value {
    let mut rows = Vec::new();
    let (mut matched, mut mismatched, mut unknown, mut exited, mut pending) = (0, 0, 0, 0, 0);
    for &tid in d.threads.keys() {
        let (mut row, c, wait) = arm_evidence(d, tid, Some(epoch), epoch, &threads.generations);
        row["pending_step_for_tid"] = json!(steps.contains_key(&tid));
        let disposition = if row["thread_owner_pid"] != d.pid || row["handle_tid"] != tid {
            unknown += 1;
            "ownership_unproven"
        } else if wait == WAIT_OBJECT_0 {
            exited += 1;
            "signaled_handle_coverage_unknown"
        } else if wait != WAIT_TIMEOUT || !d.held.contains(&tid) {
            unknown += 1;
            "context_unproven"
        } else {
            match c {
                Err(_) => {
                    unknown += 1;
                    "context_unproven"
                }
                Ok(_) if steps.contains_key(&tid) => {
                    pending += 1;
                    "pending_instruction_not_completed"
                }
                Ok(_)
                    if threads.generations.get(&tid) != Some(&epoch.generation)
                        || d.threads[&tid].armed_sites != epoch.sites =>
                {
                    mismatched += 1;
                    "generation_or_sites_mismatch"
                }
                Ok(c) if !installed_sites_match_context(&epoch.sites, &c) => {
                    mismatched += 1;
                    "debug_register_mismatch"
                }
                Ok(_) => {
                    matched += 1;
                    "matched_at_stop_only"
                }
            }
        };
        row["disposition"] = json!(disposition);
        rows.push(row);
    }
    let status = if mismatched > 0 {
        "mismatch"
    } else if unknown + exited + pending > 0 || matched == 0 {
        "incomplete"
    } else {
        "matched_at_stop"
    };
    json!({"type":"pre_restore_registers","summary":{"snapshot_status":status,"phase":epoch.phase.name(),"generation":epoch.generation,"tracked":rows.len(),"matched":matched,"mismatched":mismatched,"unknown":unknown,"signaled":exited,"pending":pending,"coverage_over_interval":"unknown","zero_hits_proves_unreached":false},"thread_checks":rows})
}
fn install_sites(
    d: &mut Debugger,
    tid: u32,
    epoch: CaptureEpoch,
    mut c: CONTEXT,
    threads: &mut CaptureThreads,
) -> Result<(), String> {
    let state = d.threads.get(&tid).ok_or("thread lost during arm")?;
    let visibility = query_thread_visibility(&state.handle, d.query_thread_start);
    if let Err(error) = visibility.require_visible() {
        return Err(reject_arm(
            d,
            json!({"tid":tid,"debugger_visibility":visibility.evidence()}),
            error,
        ));
    }
    let state = d.threads.get_mut(&tid).ok_or("thread lost during arm")?;
    [c.Dr0, c.Dr1, c.Dr2, c.Dr3] = epoch.sites.map(|s| s.unwrap_or(0));
    c.Dr7 = (c.Dr7 & !0xff & !(0xffff << 16))
        | epoch
            .sites
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_some())
            .map(|(i, _)| 1u64 << (i * 2))
            .sum::<u64>();
    c.Dr6 &= !0xf;
    // Original state was captured before any write; shared cleanup owns partial failures.
    state.armed_sites = epoch.sites;
    set_context(&state.handle, &c)?;
    let after = context(&state.handle)?;
    if !installed_sites_match_context(&epoch.sites, &after) {
        return Err("armed registers differ".into());
    }
    threads.generations.insert(tid, epoch.generation);
    Ok(())
}
fn arm(
    d: &mut Debugger,
    tid: u32,
    epoch: CaptureEpoch,
    threads: &mut CaptureThreads,
    steps: &BTreeMap<u32, Step>,
) -> Result<(), String> {
    if d.threads.contains_key(&tid) {
        return Err("duplicate capture thread create event".into());
    }
    // All attach-snapshot/create events stop the process. Validate before
    // publishing even the first native DR, after any GO wait/module change.
    epoch.validate_native_image(d)?;
    let handle = thread_handle(tid)?;
    if unsafe { GetProcessIdOfThread(handle.0) } != d.pid || unsafe { GetThreadId(handle.0) } != tid
    {
        return Err("unknown thread ownership".into());
    }
    let c = context(&handle)?;
    d.threads.insert(
        tid,
        ThreadState {
            handle,
            original: Registers::from(&c),
            returns: ReturnCorrelation::default(),
            armed_sites: [None; 4],
            property_capture: PropertyCapture::default(),
            property_owner_id: None,
            property_name: None,
            property_value_name: None,
        },
    );
    let (mut row, _, wait) = arm_evidence(d, tid, None, epoch, &threads.generations);
    row["pending_step_count"] = json!(steps.len());
    row["pending_step_for_tid"] = json!(steps.contains_key(&tid));
    if wait != WAIT_TIMEOUT || c.Dr7 & 0xff != 0 || c.EFlags & TF != 0 {
        return Err(reject_arm(
            d,
            row,
            "new capture thread is not live and free of debugger state".into(),
        ));
    }
    install_sites(d, tid, epoch, c, threads)?;
    threads.created = threads
        .created
        .checked_add(1)
        .ok_or("capture thread lifetime exhausted")?;
    threads.lifetimes.insert(tid, threads.created);
    Ok(())
}
fn capture_thread_event(
    d: &mut Debugger,
    event: &DEBUG_EVENT,
    epoch: CaptureEpoch,
    threads: &mut CaptureThreads,
    steps: &BTreeMap<u32, Step>,
) -> Result<bool, String> {
    let tid = event.dwThreadId;
    match event.dwDebugEventCode {
        CREATE_PROCESS_DEBUG_EVENT => {
            let file = unsafe { event.u.CreateProcessInfo.hFile };
            if !file.is_null() {
                unsafe {
                    CloseHandle(file);
                }
            }
            arm(d, tid, epoch, threads, steps)?;
        }
        CREATE_THREAD_DEBUG_EVENT => {
            let start = unsafe { event.u.CreateThread.lpStartAddress }.map(|p| p as usize as u64);
            d.attach_origin.note_thread_start(tid, start);
            arm(d, tid, epoch, threads, steps)?;
        }
        EXIT_THREAD_DEBUG_EVENT => {
            if let Some(step) = steps.get(&tid) {
                let mut row = if d.threads.contains_key(&tid) {
                    arm_evidence(d, tid, Some(epoch), epoch, &threads.generations).0
                } else {
                    json!({"type":"arm_rejected","tid":tid,"raw_context_error":"unknown exiting thread handle"})
                };
                row["event_code"] = json!(event.dwDebugEventCode);
                row["exit_event_code"] = json!(unsafe { event.u.ExitThread.dwExitCode });
                row["pending_step"] = json!({"capture_seq":step.capture["capture_seq"],"expected_rip":step.expected,"armed_eflags":step.armed_eflags});
                return Err(reject_arm(
                    d,
                    row,
                    "thread exited before original instruction completion".into(),
                ));
            }
            if !d.threads.contains_key(&tid) || !threads.generations.contains_key(&tid) {
                return Err("exit event without a tracked capture thread".into());
            }
            d.attach_origin.note_thread_exit(tid);
            // A real EXIT_THREAD event retires the pinned handle, never a guessed TID.
            d.held.remove(&tid);
            d.threads.remove(&tid);
            threads.generations.remove(&tid);
            threads.lifetimes.remove(&tid);
            threads.exited += 1;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
enum PhaseBarrier {
    Deferred,
    Event(DEBUG_EVENT),
    Switched(Vec<Value>),
}
fn phase_barrier(
    d: &mut Debugger,
    steps: &BTreeMap<u32, Step>,
    from: CaptureEpoch,
    to: CaptureEpoch,
    threads: &mut CaptureThreads,
) -> Result<PhaseBarrier, String> {
    if !steps.is_empty() || d.cleanup_event.is_some() {
        return Ok(PhaseBarrier::Deferred);
    }
    if to.generation
        != from
            .generation
            .checked_add(1)
            .ok_or("capture generation overflow")?
        || to.phase == from.phase
    {
        return Err("invalid capture phase generation".into());
    }
    d.freeze()?;
    let mut event = DEBUG_EVENT::default();
    if unsafe { WaitForDebugEventEx(&mut event, 0) } != 0 {
        d.cleanup_event = Some((event, DBG_EXCEPTION_NOT_HANDLED));
        d.resume()?;
        return Ok(PhaseBarrier::Event(event));
    }
    if std::io::Error::last_os_error().raw_os_error() != Some(121) {
        return Err(win_error("phase drain WaitForDebugEventEx"));
    }
    rearm_frozen_threads(d, steps, from, to, threads).map(PhaseBarrier::Switched)
}
fn rearm_frozen_threads(
    d: &mut Debugger,
    steps: &BTreeMap<u32, Step>,
    from: CaptureEpoch,
    to: CaptureEpoch,
    threads: &mut CaptureThreads,
) -> Result<Vec<Value>, String> {
    if !steps.is_empty() || d.cleanup_event.is_some() {
        return Err("phase commit with pending instruction/event".into());
    }
    // This is after freeze AND the zero-time pending-event drain. Keep the
    // native check beside its first use, rather than before the stop boundary.
    to.validate_native_image(d)?;
    let mut prepared = Vec::new();
    let mut evidence = Vec::new();
    for tid in d.threads.keys().copied().collect::<Vec<_>>() {
        let (mut row, c, wait) = arm_evidence(d, tid, Some(from), to, &threads.generations);
        row["pending_step_count"] = json!(steps.len());
        row["pending_step_for_tid"] = json!(steps.contains_key(&tid));
        if wait == WAIT_OBJECT_0
            && row["exit_code"].is_u64()
            && threads.generations.contains_key(&tid)
        {
            // Keep this terminated handle tracked until its EXIT event (or
            // detach), but never read it as a live DR set or write/resume it.
            row["disposition"] = json!("signaled_handle_awaiting_exit_event");
            d.held.remove(&tid);
            evidence.push(row);
            continue;
        }
        let c = match c {
            Ok(c) => c,
            Err(e) => return Err(reject_arm(d, row, e)),
        };
        let state = &d.threads[&tid];
        if wait != WAIT_TIMEOUT
            || !d.held.contains(&tid)
            || row["thread_owner_pid"] != d.pid
            || row["handle_tid"] != tid
            || threads.generations.get(&tid) != Some(&from.generation)
            || state.armed_sites != from.sites
            || !installed_sites_match_context(&state.armed_sites, &c)
        {
            return Err(reject_arm(
                d,
                row,
                "prior capture debug-register ownership changed".into(),
            ));
        }
        if row["debugger_visibility"]["hidden"] != false {
            return Err(reject_arm(
                d,
                row,
                "phase preflight cannot prove thread visible; DR write refused".into(),
            ));
        }
        row["disposition"] = json!("live_held_verified");
        evidence.push(row);
        prepared.push((tid, c));
    }
    // Preflight ALL threads before changing the first one. No partial phase
    // executes: any write/readback failure goes through shared restoration.
    for (tid, c) in prepared {
        install_sites(d, tid, to, c, threads)?;
    }
    Ok(evidence)
}
fn read_pointer(game: &Handle, address: u64) -> Result<u64, String> {
    Ok(u64::from_le_bytes(read_exact::<8>(game, address)?))
}
fn mark(d: &mut Debugger, mut row: Value) -> Result<(), String> {
    row["sequence"] = json!(d.output.rows + 1);
    d.stamp(&mut row);
    let disk = d.output.row(row.clone());
    if row["type"] == "order_stop" {
        // A full trace disk must not suppress the independently verified
        // restoration receipt delivered to the parent over its pipe.
        row["trace_write_error"] = json!(disk.as_ref().err());
        emit(&row)?;
    }
    disk
}

fn emit(row: &Value) -> Result<(), String> {
    writeln!(std::io::stdout(), "{row}")
        .and_then(|_| std::io::stdout().flush())
        .map_err(|e| e.to_string())
}

const THREAD_NAME_EXCEPTION: i32 = 0x406d1388;
// Microsoft pack(8) THREADNAME_INFO on Win64 is 24 bytes / 3 ULONG_PTRs.
// The shipped Bink producer passes count=6 for that same 24-byte prefix;
// live04's dump confirms six words. Only this observed legacy count is added.
// The upper DWORD of word 0 is padding, not a required zero field.
fn thread_name_notice(d: &Debugger, event: &DEBUG_EVENT) -> Result<Value, String> {
    let ex = unsafe { event.u.Exception }.ExceptionRecord;
    if event.dwProcessId != d.pid
        || ex.ExceptionCode != THREAD_NAME_EXCEPTION
        // Windows adds EXCEPTION_SOFTWARE_ORIGINATE (SDK winnt.h) to the
        // observed record even when RaiseException was called with flags=0.
        || ex.ExceptionFlags & !0x80 != 0
        || !matches!(ex.NumberParameters, 3 | 6)
    {
        return Err("not the Win64 thread-name notification shape".into());
    }
    let words = &ex.ExceptionInformation;
    if words[0] as u32 != 0x1000 || (words[2] as u64 >> 32) != 0 || words[1] == 0 {
        return Err("invalid thread-name fields".into());
    }
    let target = if words[2] as u32 == u32::MAX {
        event.dwThreadId
    } else {
        words[2] as u32
    };
    for tid in [event.dwThreadId, target] {
        let thread = d
            .threads
            .get(&tid)
            .ok_or("thread-name thread is not tracked")?;
        if unsafe { GetProcessIdOfThread(thread.handle.0) } != d.pid
            || unsafe { GetThreadId(thread.handle.0) } != tid
            || unsafe { WaitForSingleObject(thread.handle.0, 0) } != WAIT_TIMEOUT
        {
            return Err("thread-name thread ownership or lifetime differs".into());
        }
    }
    let mut name = Vec::new();
    for i in 0..256u64 {
        let address = (words[1] as u64)
            .checked_add(i)
            .ok_or("thread-name address overflow")?;
        let byte = read_exact::<1>(&d.game, address)?[0];
        if byte == 0 {
            return Ok(
                json!({"target_tid":target,"name_bytes":name,"name_address":words[1],"layout":if ex.NumberParameters==3 {"Win64-pack8-3-ULONG_PTR"} else {"Civ6-Bink-Win64-pack8-prefix-count6"},"padding_ignored":(words[0] as u64)>>32,"trailing_words_uninterpreted":if ex.NumberParameters==6 {&words[3..6]}else{&words[3..3]}}),
            );
        }
        name.push(byte);
    }
    Err("thread-name is not terminated within 256 bytes".into())
}

fn qualify_exception(
    d: &mut Debugger,
    event: &DEBUG_EVENT,
    steps: &BTreeMap<u32, Step>,
    sites: &[u64],
) -> Result<i32, String> {
    if event.dwDebugEventCode != EXCEPTION_DEBUG_EVENT {
        return Ok(DBG_CONTINUE);
    }
    let info = unsafe { event.u.Exception };
    let tid = event.dwThreadId;
    let address = info.ExceptionRecord.ExceptionAddress as u64;
    match info.ExceptionRecord.ExceptionCode {
        THREAD_NAME_EXCEPTION if thread_name_notice(d, event).is_ok() => {
            if steps.contains_key(&tid) {
                d.last_dispatch =
                    DispatchOutcome::Ambiguous("thread-name interrupted pending step");
                return Err("thread-name cannot complete or discard a pending instruction".into());
            }
            // A validated naming notification has no instruction-completion
            // meaning. Keep every other thread's pending step untouched.
            return Ok(DBG_CONTINUE);
        }
        EXCEPTION_SINGLE_STEP => {
            d.last_dispatch = DispatchOutcome::Ambiguous("order exception ownership unresolved");
            let state = d.threads.get(&tid).ok_or("unknown exception thread")?;
            if unsafe { GetProcessIdOfThread(state.handle.0) } != d.pid {
                return Err("exception thread ownership changed".into());
            }
            let c = context(&state.handle)?;
            if !installed_sites_match_context(&state.armed_sites, &c) || info.dwFirstChance == 0 {
                return Err("exception register identity differs".into());
            }
            if let Some(step) = steps.get(&tid) {
                plan::verify_step(
                    step.expected,
                    address,
                    c.Rip,
                    c.Dr6,
                    c.EFlags,
                    step.armed_eflags,
                )?;
            } else {
                if c.EFlags & TF != 0
                    || plan::classify_hit(
                        address,
                        c.Rip,
                        [c.Dr0, c.Dr1, c.Dr2, c.Dr3],
                        c.Dr7,
                        c.Dr6,
                        sites,
                    )?
                    .is_none()
                {
                    return Err("unowned order exception".into());
                }
            }
            d.last_dispatch = DispatchOutcome::OwnedProven(OwnedTrap::PropertyBefore);
        }
        EXCEPTION_BREAKPOINT => {
            if steps.contains_key(&tid) {
                d.last_dispatch = DispatchOutcome::Ambiguous("breakpoint interrupted step");
                return Err("breakpoint interrupted step".into());
            }
            let observed = inspect_thread_origin(d.pid, tid, d.query_thread_start);
            d.last_attach_breakpoint_owned =
                d.classify_attach_breakpoint(tid, address, info.dwFirstChance != 0, observed);
        }
        _ => {}
    }
    d.cleanup_event = Some((*event, d.status_after_error(event)));
    Ok(d.status_after_error(event))
}

fn exception_evidence(d: &Debugger, event: &DEBUG_EVENT, steps: &BTreeMap<u32, Step>) -> Value {
    let info = unsafe { event.u.Exception };
    let tid = event.dwThreadId;
    let mut row = json!({"type":"debug_exception","tid":tid,"code":info.ExceptionRecord.ExceptionCode,"address":info.ExceptionRecord.ExceptionAddress as u64,"first_chance":info.dwFirstChance!=0,"expected_owner_pid":d.pid,"pending_step_count":steps.len()});
    let ex = &info.ExceptionRecord;
    row["exception_flags"] = json!(ex.ExceptionFlags);
    row["number_parameters"] = json!(ex.NumberParameters);
    row["exception_information"] = json!(
        &ex.ExceptionInformation
            [..(ex.NumberParameters as usize).min(ex.ExceptionInformation.len())]
    );
    row["exception_information_truncated"] =
        json!(ex.NumberParameters as usize > ex.ExceptionInformation.len());
    if ex.ExceptionCode == THREAD_NAME_EXCEPTION {
        match thread_name_notice(d, event) {
            Ok(notice) => row["thread_name_notice"] = notice,
            Err(error) => row["thread_name_rejected"] = json!(error),
        }
    }
    if let Some(step) = steps.get(&tid) {
        row["pending_step"] = json!({"capture_seq":step.capture["capture_seq"],"site":step.site.name,"expected_rip":step.expected,"phase":step.phase.name(),"armed_eflags":step.armed_eflags});
    }
    match d.threads.get(&tid) {
        Some(state) => {
            row["thread_owner_pid"] = json!(unsafe { GetProcessIdOfThread(state.handle.0) });
            row["armed_sites"] = json!(state.armed_sites);
            match context(&state.handle) {
                Ok(c) => {
                    row["raw_context"] = json!({"context_flags":c.ContextFlags,"rax":c.Rax,"rbx":c.Rbx,"rcx":c.Rcx,"rdx":c.Rdx,"r8":c.R8,"r9":c.R9,"rsp":c.Rsp,"rip":c.Rip,"eflags":c.EFlags,"dr0":c.Dr0,"dr1":c.Dr1,"dr2":c.Dr2,"dr3":c.Dr3,"dr6":c.Dr6,"dr7":c.Dr7})
                }
                Err(error) => row["raw_context_error"] = json!(error),
            }
        }
        None => row["raw_context_error"] = json!("unknown exception thread; no handle reopened"),
    }
    row
}

fn classify_then_record(
    d: &mut Debugger,
    event: &DEBUG_EVENT,
    steps: &BTreeMap<u32, Step>,
    sites: &[u64],
    record: impl FnOnce(&mut Debugger, Value) -> std::io::Result<()>,
) -> Result<(), String> {
    // Obtain raw evidence without I/O. Decide ownership/continuation before
    // any fallible write, including when qualification itself fails.
    let mut row = exception_evidence(d, event, steps);
    let qualified = qualify_exception(d, event, steps, sites);
    let status = qualified
        .as_ref()
        .copied()
        .unwrap_or_else(|_| d.status_after_error(event));
    d.cleanup_event = Some((*event, status));
    row["qualification_error"] = json!(qualified.as_ref().err());
    row["continue_status"] = json!(status);
    row["qualification_succeeded"] = json!(qualified.is_ok());
    let fallback = row.clone();
    let recorded = record(d, row);
    if let Err(error) = &recorded {
        let _ = writeln!(
            std::io::stderr(),
            "{}",
            json!({"type":"exception_record_failure","error":error.to_string(),"exception":fallback})
        );
    }
    qualified
        .map(|_| ())
        .and(recorded.map_err(|e| e.to_string()))
}

fn cleanup_order(d: &mut Debugger) -> Result<(), String> {
    // Native default classification does not know the naming protocol. Preserve
    // the status already fixed before recording, including on disk failure.
    let pending = d.cleanup_event;
    d.cleanup(pending.as_ref().map(|(event, status)| (event, *status)))
}

fn core_module(pid: u32) -> Result<Option<Module>, String> {
    match module_named(pid, "GameCore_XP2_FinalRelease.dll") {
        Ok(m) => Ok(Some(m)),
        Err(e) if e == format!("GameCore_XP2_FinalRelease.dll is not loaded in process {pid}") => {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

fn chain_descendant(pid: u32, chain_pid: u32) -> Result<(), String> {
    let mut ancestor = pid;
    for _ in 0..8 {
        if ancestor == chain_pid {
            return Ok(());
        }
        ancestor = parent_pid(ancestor)?;
    }
    Err("controller is not descended from live_chain".into())
}

// Nonblocking pipe reads let the owned reader join, including after a timeout.
fn commands(buffer: &mut Vec<u8>) -> Result<Vec<String>, String> {
    plan::protocol::commands(buffer)
}

struct Reader {
    done: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}
impl Drop for Reader {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Release);
        if let Some(t) = self.join.take() {
            let _ = t.join();
        }
    }
}
fn reader(ack: Arc<AtomicBool>) -> (Reader, Receiver<Result<String, String>>) {
    let done = Arc::new(AtomicBool::new(false));
    let worker = done.clone();
    let (tx, rx) = mpsc::channel();
    let join = thread::spawn(move || {
        let mut buffer = Vec::new();
        while !worker.load(Ordering::Acquire) {
            match commands(&mut buffer) {
                Ok(list) => {
                    for cmd in list {
                        if cmd == "game_close_requested" {
                            ack.store(true, Ordering::SeqCst);
                        } else {
                            let eof = cmd == "eof";
                            if tx.send(Ok(if eof { "stop".into() } else { cmd })).is_err() {
                                return;
                            }
                            if eof {
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
    });
    (
        Reader {
            done,
            join: Some(join),
        },
        rx,
    )
}

// Diagnostic snapshot only: no attach, suspension, DR/context write or setter.
// Reading false at this instant never admits hardware observation afterward.
pub fn audit(req: Request) -> Result<(), String> {
    let chain = OrderChain::pin(req.chain_pid, req.chain_created)?;
    let lock = Path::new("D:/CC/Tools/civ6-live/civ6.lock");
    verify_order_lock(lock, req.chain_pid, &chain)?;
    let controller = process(req.controller_pid, false)?;
    if process_created(&controller)? != req.controller_created
        || !alive(&controller)?
        || parent_pid(std::process::id())? != req.controller_pid
        || std::env::var("CIV6_LIVE_CHAIN_PID")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            != Some(req.chain_pid)
    {
        return Err("audit controller or inherited chain identity differs".into());
    }
    chain_descendant(req.controller_pid, req.chain_pid)?;
    let game = exact_process(
        req.pid,
        req.created,
        Path::new(crate::PINNED_DX11_EXE),
        true,
    )?;
    if req.created < req.chain_created {
        return Err("audit game predates owning chain".into());
    }
    let exe = module_named(req.pid, "CivilizationVI.exe")?;
    if !is_pinned_dx11_exe(&exe.path) {
        return Err("audit EXE path differs".into());
    }
    plan::disk_sites(&fs::read(&exe.path).map_err(|e| e.to_string())?)?;
    let core_path = Path::new(GAME_ROOT).join(MODULE_REL);
    let _core_pin = pin_hashed_image_file(&core_path, plan::CORE_SHA, "audit Core")?;
    let (_, query, ntdll) = resolve_attach_origin(req.pid)?;
    let _source_pin =
        pin_hashed_image_file(&req.source_dll, &req.source_dll_sha, "audit source DLL")?;
    let rows = audit_threads(req.pid, query)?;
    let core = core_module(req.pid)?;
    if core
        .as_ref()
        .is_some_and(|m| !same_path(&m.path, &core_path))
    {
        return Err("audit Core module path differs".into());
    }
    verify_order_lock(lock, req.chain_pid, &chain)?;
    if !alive(&game)? || process_created(&game)? != req.created || !alive(&controller)? {
        return Err("audit identity changed during snapshot".into());
    }
    let report = json!({"type":"order_thread_audit","schema":plan::SCHEMA,
        "observed_unix_ms":SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d|d.as_millis()),
        "source_dll_sha256":req.source_dll_sha,"observer_source":plan::source_identity()["compiled_source_sha256"],
        "game_pid":req.pid,"game_created_filetime":req.created,
        "chain_pid":req.chain_pid,"chain_created_filetime":req.chain_created,
        "controller_pid":req.controller_pid,"controller_created_filetime":req.controller_created,
        "threads":rows,"initial_core_base":core.as_ref().map(|m|m.base),
        "ntdll_disk_sha256":ntdll.disk_sha256,"ntdll_query_rva":ntdll.query_rva,
        "snapshot_complete_over_interval":false,"future_thread_visibility_proven":false,
        "hardware_capture_admitted":false,"debug_attach_calls":0,"debug_register_writes":0,
        "thread_suspend_calls":0,"thread_hide_set_calls":0,"memory_writes":0,"dll_install_calls":0,
        "native_event_root_identity":"unknown","rebuild_authorized":false,"root_generation_verified":false,
        "root_activation_allowed":false,"all_producer_domains_covered":false,"same_set_wait_proven":false,"pending_publication_drained":false,
        "native_quiescence_proven":false,"runtime_activation_allowed":false});
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&req.output)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut file, &report).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    emit(&report)
}
fn audit_threads(pid: u32, query: NtQueryThreadFn) -> Result<Vec<Value>, String> {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    let snapshot = Handle::new(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) },
        "audit thread snapshot",
    )?;
    let mut entry: THREADENTRY32 = unsafe { zeroed() };
    entry.dwSize = size_of::<THREADENTRY32>() as u32;
    let mut found = unsafe { Thread32First(snapshot.0, &mut entry) };
    let mut rows = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while found != 0 {
        if entry.th32OwnerProcessID == pid {
            if rows.len() >= 1024 || Instant::now() >= deadline {
                return Err("audit thread bound exceeded".into());
            }
            let tid = entry.th32ThreadID;
            let handle = Handle::new(
                unsafe { OpenThread(THREAD_QUERY_INFORMATION | THREAD_SYNCHRONIZE, 0, tid) },
                "audit OpenThread",
            );
            let mut row = json!({"tid":tid,"expected_owner_pid":pid,"read_only":true,"visibility":"unknown","debugger_visibility":null,
                "observed_unix_ms":SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d|d.as_millis())});
            match handle {
                Err(error) => row["inspection_error"] = json!(error),
                Ok(handle) => {
                    row["owner_pid"] = json!(unsafe { GetProcessIdOfThread(handle.0) });
                    row["handle_tid"] = json!(unsafe { GetThreadId(handle.0) });
                    row["handle_wait"] = json!(unsafe { WaitForSingleObject(handle.0, 0) });
                    if row["owner_pid"] != pid
                        || row["handle_tid"] != tid
                        || row["handle_wait"] != WAIT_TIMEOUT
                    {
                        row["inspection_error"] = json!("thread ownership/liveness unproven");
                    } else {
                        row["debugger_visibility"] =
                            query_thread_visibility(&handle, query).evidence();
                        row["win32_start_candidate"] =
                            json!(query_win32_start(&handle, query).address);
                        row["handle_wait_after_query"] =
                            json!(unsafe { WaitForSingleObject(handle.0, 0) });
                        if row["handle_wait_after_query"] == WAIT_TIMEOUT {
                            row["visibility"] = row["debugger_visibility"]["visibility"].clone();
                        }
                    }
                }
            }
            rows.push(row);
        }
        found = unsafe { Thread32Next(snapshot.0, &mut entry) };
    }
    if std::io::Error::last_os_error().raw_os_error() != Some(18) {
        return Err(win_error("audit Thread32Next"));
    }
    Ok(rows)
}

pub fn observe(req: Request) -> Result<(), String> {
    // A query cannot prevent a thread from hiding after its snapshot. Until
    // that delivery gap is controlled, production must not arm any game DR.
    if !cfg!(test) {
        return Err("live hardware capture withheld: future thread debugger visibility unproven; use audit-thread-visibility".into());
    }
    // Resolve authority before attach. No lock acquisition, reservation, launch,
    // injection, DLL loading, memory setter or runtime API is present here.
    let chain = OrderChain::pin(req.chain_pid, req.chain_created)?;
    let lock = Path::new("D:/CC/Tools/civ6-live/civ6.lock");
    verify_order_lock(lock, req.chain_pid, &chain)?;
    let controller = process(req.controller_pid, false)?;
    if process_created(&controller)? != req.controller_created
        || !alive(&controller)?
        || parent_pid(std::process::id())? != req.controller_pid
    {
        return Err("controller is not the pinned OS parent".into());
    }
    chain_descendant(req.controller_pid, req.chain_pid)?;
    let inherited =
        std::env::var("CIV6_LIVE_CHAIN_PID").map_err(|_| "missing inherited chain identity")?;
    if inherited.parse::<u32>().ok() != Some(req.chain_pid) {
        return Err("inherited chain identity differs".into());
    }
    let game = exact_process(
        req.pid,
        req.created,
        Path::new(crate::PINNED_DX11_EXE),
        true,
    )?;
    if req.created < req.chain_created {
        return Err("game predates owning live_chain".into());
    }
    let exe = module_named(req.pid, "CivilizationVI.exe")?;
    if !is_pinned_dx11_exe(&exe.path) {
        return Err("loaded EXE path differs".into());
    }
    let disk = fs::read(&exe.path).map_err(|e| e.to_string())?;
    let mut static_proof = plan::disk_sites(&disk)?;
    let core_path = Path::new(GAME_ROOT).join(MODULE_REL);
    // Validate once while denying writes/deletion. Rehashing the complete PE
    // on every attach thread/DLL event can exhaust the fixed readiness budget.
    let core_pin = pin_hashed_image_file(&core_path, plan::CORE_SHA, "Core")?;
    static_proof["core_file_pinned"] = json!(true);
    static_proof["native_outer"] =
        plan::disk_native_sites(&fs::read(&core_path).map_err(|e| e.to_string())?)?;
    static_proof["native_shutdown"] =
        plan::disk_shutdown_sites(&fs::read(&core_path).map_err(|e| e.to_string())?)?;
    if !req.source_dll.is_absolute()
        || req
            .source_dll
            .extension()
            .is_none_or(|s| !s.eq_ignore_ascii_case("dll"))
        || sha256(&req.source_dll)? != req.source_dll_sha.to_ascii_lowercase()
    {
        return Err("source DLL hash/path mismatch".into());
    }
    // Hold source against write/delete for the complete capture lifetime.
    use std::os::windows::fs::OpenOptionsExt;
    let source_pin = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&req.source_dll)
        .map_err(|e| e.to_string())?;
    if sha256(&req.source_dll)? != req.source_dll_sha.to_ascii_lowercase() {
        return Err("source DLL changed while pinning".into());
    }
    for site in plan::FRONT.iter().chain(plan::LOAD.iter()) {
        instruction(&game, exe.base, *site)?;
    }
    // Refuse missing native capability before attaching the debugger.
    let initial_core = core_module(req.pid)?;
    if initial_core
        .as_ref()
        .is_some_and(|m| !same_path(&m.path, &core_path))
    {
        return Err("initial GameCore path differs".into());
    }
    let capture_core_base = initial_core.as_ref().map(|core| core.base);
    if req.phase.is_native() {
        for site in *req.phase.sites() {
            instruction(&game, site.image.base(exe.base, capture_core_base)?, site)?;
        }
    }
    let game_guard = exact_process(req.pid, req.created, &exe.path, false)?;
    let output = Jsonl::new(&req.output)?;
    let token = std::env::var("TODO880_ATTACH_TOKEN").map_err(|_| "parent attach token missing")?;
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid attach token".into());
    }
    let identity = json!({"type":"attach_ready","schema":plan::SCHEMA,"game_pid":req.pid,"game_created_filetime":req.created,"chain_pid":req.chain_pid,"chain_created_filetime":req.chain_created,"controller_pid":req.controller_pid,"controller_created_filetime":req.controller_created,"source_dll_sha256":req.source_dll_sha,"observer_pid":std::process::id(),"attach_token":token});
    emit(&identity)?;
    let mut admission = plan::protocol::Admission::new(identity, token);
    admission.await_go()?;
    verify_order_lock(lock, req.chain_pid, &chain)?;
    if !alive(&controller)? || !alive(&game_guard)? || process_created(&game_guard)? != req.created
    {
        return Err("identity changed after GO".into());
    }
    // Attach in existing non-hardware mode. Only this explicit capability then
    // arms the fixed EXE sites; old live hardware modes stay rejected.
    let mut d = admission.enter_and_call(|| {
        Debugger::attach(
            req.pid,
            game,
            exe.base,
            exe.size,
            output,
            ProbeMode::Lifecycle,
        )
    })?;
    d.mode = ProbeMode::Property; // existing restoration walks our pinned threads
    d.order_core_pin = Some(core_pin);
    d.attach_ready_deadline = Some(Instant::now() + ATTACH_READY_BUDGET);
    let initial_context = read_pointer(&d.game, exe.base + 0x22d60c0)?;
    let mut module_identities = BTreeMap::new();
    if let Some(core) = &initial_core {
        module_identities.insert(core.base, context_facts::module_row(core));
    }
    mark(
        &mut d,
        json!({"type":"order_identity","schema":plan::SCHEMA,"game_pid":req.pid,"game_created_filetime":req.created,"chain_pid":req.chain_pid,"chain_created_filetime":req.chain_created,"controller_pid":req.controller_pid,"controller_created_filetime":req.controller_created,"source_dll":req.source_dll,"source_dll_sha256":req.source_dll_sha,"exe_base":exe.base,"capture_phase":req.phase.name(),"front_prefix":req.front_prefix,"load_prefix":req.load_prefix,"static_proof":static_proof,"initial_core_base":initial_core.as_ref().map(|m|m.base),"initial_context":initial_context,"first_factory_since_process_start":"unknown","gameplay_lua_identity":"unknown","memory_writes":0,"dll_install_calls":0}),
    )?;
    let mut phase = req.phase;
    let mut epoch = CaptureEpoch::new(phase, 0, exe.base, capture_core_base, &req)?;
    let mut capture_threads = CaptureThreads::default();
    let mut native_core_unloaded = false;
    let mut desired = phase;
    let mut steps: BTreeMap<u32, Step> = BTreeMap::new();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let (input_reader, input) = reader(d.close_ack.clone());
    let started = Instant::now();
    let mut stop = admission.stop_requested();
    let mut hits = 0;
    let mut drain_deadline = None;

    let mut armed_windows = BTreeMap::<String, u64>::new();
    for s in plan::prefix(phase, req.count(phase))? {
        armed_windows.insert(s.name.into(), 1);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
        loop {
            for command in input.try_iter() {
                let command = command?;
                match command.as_str() {
                    "stop" => stop = true,
                    "phase front" => desired = Phase::Front,
                    "phase load" => desired = Phase::Load,
                    "phase native-outer" => desired = Phase::NativeOuter,
                    "phase native-shutdown" => desired = Phase::NativeShutdown,
                    "game_close_requested" => d.close_ack.store(true, Ordering::SeqCst),
                    _ => return Err("unknown command; use phase front, load, native-outer, native-shutdown or stop".into()),
                }
            }
            if stop || started.elapsed() >= Duration::from_secs(req.seconds) || hits >= req.max_hits
            {
                if steps.is_empty() {
                    break;
                }
                let deadline =
                    drain_deadline.get_or_insert_with(|| Instant::now() + Duration::from_secs(2));
                if Instant::now() >= *deadline {
                    return Err("pending instruction did not complete before drain deadline".into());
                }
            }
            verify_order_lock(lock, req.chain_pid, &chain)?;
            if !alive(&controller)? || !alive(&game_guard)? {
                return Err("pinned owner or game exited".into());
            }
            if d.attach_deadline_expired() {
                return Err("attach handshake deadline".into());
            }
            let mut event = DEBUG_EVENT::default();
            if unsafe { WaitForDebugEventEx(&mut event, 50) } == 0 {
                if std::io::Error::last_os_error().raw_os_error() != Some(121) {
                    return Err(win_error("WaitForDebugEventEx"));
                }
                if desired != phase && steps.is_empty() && d.attach_origin.accepted {
                    if epoch.generation >= 8 {return Err("phase change bound exceeded".into());}
                    if desired.is_native() {
                        if native_core_unloaded { return Err("native capture Core lifetime ended since attach".into()); }
                        let core = core_module(req.pid)?.ok_or("native capture requires loaded Core")?;
                        if Some(core.base) != capture_core_base || !same_path(&core.path, &core_path) || d.order_core_pin.is_none() {
                            return Err("native capture Core changed since attach".into());
                        }
                    }
                    let next = CaptureEpoch::new(desired, epoch.generation + 1, exe.base, capture_core_base, &req)?;
                    for site in plan::prefix(desired, req.count(desired))? {instruction(&d.game,site.image.base(exe.base,capture_core_base)?,*site)?;}
                    match phase_barrier(&mut d,&steps,epoch,next,&mut capture_threads)? {
                        PhaseBarrier::Deferred => continue,
                        PhaseBarrier::Event(pending) => event=pending,
                        PhaseBarrier::Switched(thread_checks) => {
                            mark(&mut d,json!({"type":"phase_gap","old":phase.name(),"new":desired.name(),"generation":next.generation,"thread_checks":thread_checks,"unarmed_sites":"not_observed","first_factory_since_process_start":"unknown"}))?;
                            epoch=next;
                            phase=epoch.phase;
                            for site in plan::prefix(phase,req.count(phase))? {*armed_windows.entry(site.name.into()).or_default()+=1;}
                            d.resume()?;
                            emit(&json!({"phase_ready":phase.name(),"pid":req.pid}))?;
                            continue;
                        }
                    }
                } else {
                    continue;
                }
            }
            d.event_ordinal += 1;
            d.last_dispatch = DispatchOutcome::Unrelated;
            d.last_attach_breakpoint_owned = false;
            d.cleanup_event = Some((event, DBG_EXCEPTION_NOT_HANDLED));
            if event.dwProcessId != req.pid {
                return Err("foreign debug event".into());
            }
            let tid = event.dwThreadId;
            let mut status = DBG_CONTINUE;
            let mut announce_ready=false;
            match event.dwDebugEventCode {
                CREATE_PROCESS_DEBUG_EVENT | CREATE_THREAD_DEBUG_EVENT | EXIT_THREAD_DEBUG_EVENT => {
                    capture_thread_event(&mut d,&event,epoch,&mut capture_threads,&steps)?;
                }
                LOAD_DLL_DEBUG_EVENT => {
                    let info = unsafe { event.u.LoadDll };
                    if !info.hFile.is_null() {
                        unsafe {
                            CloseHandle(info.hFile);
                        }
                    }
                    let attach_snapshot = !d.attach_origin.accepted;
                    let module_base = info.lpBaseOfDll as u64;
                    // At attach only the known Core identity is needed. For
                    // real reloads retain the module identity BEFORE unload.
                    if !attach_snapshot {
                        let identity = context_facts::module_at(req.pid, module_base)
                            .map(|m| context_facts::module_row(&m))
                            .unwrap_or_else(|e| json!({"base":module_base,"identity":"unknown","error":e}));
                        module_identities.insert(module_base, identity);
                    }
                    mark(
                        &mut d,
                        json!({"type":if attach_snapshot {"module_attach_snapshot"}else{"module_load"},"tid":tid,"base":module_base,"module_identity":module_identities.get(&module_base),"source":"real_debug_event","load_since_ready":!attach_snapshot}),
                    )?;
                    if let Some(core) = core_module(req.pid)? {
                        if !same_path(&core.path, &core_path)
                            || d.order_core_pin.is_none()
                        {
                            return Err("loaded GameCore identity differs".into());
                        }
                    }
                    epoch.validate_native_image(&d)?;
                }
                UNLOAD_DLL_DEBUG_EVENT => {
                    let module_base=unsafe {event.u.UnloadDll.lpBaseOfDll} as u64;
                    if Some(module_base) == capture_core_base {
                        // Reuse of base/path/SHA after unload is a new lifetime,
                        // including when a non-native phase was armed at unload.
                        native_core_unloaded = true;
                        if phase.is_native() {
                            return Err("native capture Core unloaded; no lifetime certificate".into());
                        }
                    }
                    let prior_identity=module_identities.remove(&module_base);
                    mark(
                        &mut d,
                        json!({"type":"module_unload","tid":tid,"base":module_base,"prior_module_identity":prior_identity,"identity_source":"last_observed_load_or_initial_core","source":"real_debug_event"}),
                    )?;
                }
                EXIT_PROCESS_DEBUG_EVENT => {
                    d.continue_pending_with_status(DBG_CONTINUE)?;
                    return Err("game exited during capture".into());
                }
                EXCEPTION_DEBUG_EVENT => {
                    let info = unsafe { event.u.Exception };
                    let address = info.ExceptionRecord.ExceptionAddress as u64;
                    let sites:Vec<_>=epoch.sites.iter().flatten().copied().collect();
                    classify_then_record(&mut d,&event,&steps,&sites,|d,mut row|{row["phase"]=json!(phase.name());mark(d,row).map_err(std::io::Error::other)})?;
                    if info.ExceptionRecord.ExceptionCode == EXCEPTION_BREAKPOINT {
                        if steps.contains_key(&tid) {
                            d.last_dispatch = DispatchOutcome::Ambiguous(
                                "breakpoint interrupted original instruction",
                            );
                            return Err("breakpoint interrupted original instruction".into());
                        }
                        let owned = d.last_attach_breakpoint_owned;
                        if owned {
                            d.mark_attach_ready()?;
                            d.attach_ready_deadline = None;
                            announce_ready=true;
                        } else {
                            status = DBG_EXCEPTION_NOT_HANDLED;
                        }
                    } else if info.ExceptionRecord.ExceptionCode == EXCEPTION_SINGLE_STEP {
                        d.last_dispatch =
                            DispatchOutcome::Ambiguous("order trap not yet qualified");
                        let state = d
                            .threads
                            .get(&tid)
                            .ok_or("unknown hardware-breakpoint thread handle")?;
                        if unsafe { GetProcessIdOfThread(state.handle.0) } != req.pid {
                            return Err("thread owner changed".into());
                        }
                        let mut c = context(&state.handle)?;
                        if !installed_sites_match_context(&state.armed_sites, &c) {
                            return Err("debug-register set differs at capture trap".into());
                        }
                        if let Some(step) = steps.get(&tid) {
                            if info.dwFirstChance == 0 {
                                return Err(
                                    "second-chance single-step is not owned completion".into()
                                );
                            }
                            plan::verify_step(step.expected, address, c.Rip, c.Dr6,c.EFlags,step.armed_eflags)?;
                            d.last_dispatch =
                                DispatchOutcome::OwnedProven(OwnedTrap::PropertyAfter);
                            c.EFlags &= !TF;
                            c.Dr6 &= !((1 << 14) | 15);
                            c.EFlags |= RF;
                            set_context(&state.handle, &c)?;
                            let step = steps.remove(&tid).unwrap();
                            let mut row = step.capture;
                            row["type"] = json!("instruction_completed");
                            row["completion_rip"] = json!(c.Rip);
                            row["completion_rsp"] = json!(c.Rsp);
                            row["original_instruction_executions"] = json!(1);
                            row["phase"] = json!(step.phase.name());
                            *counts.entry(step.site.name.into()).or_default() += 1;
                            mark(&mut d, row)?;
                        } else {
                            let sites = plan::prefix(phase, req.count(phase))?;
                            let addresses: Vec<_> = epoch.sites.iter().flatten().copied().collect();
                            match plan::classify_hit(
                                address,
                                c.Rip,
                                [c.Dr0, c.Dr1, c.Dr2, c.Dr3],
                                c.Dr7,
                                c.Dr6,
                                &addresses,
                            )? {
                                None => {
                                    return Err("single-step is not owned by the current capture phase".into());
                                }
                                Some(i) => {
                                    if c.EFlags & TF != 0 || info.dwFirstChance == 0 {
                                        return Err("unexpected stepping state".into());
                                    }
                                    d.last_dispatch =
                                        DispatchOutcome::OwnedProven(OwnedTrap::PropertyBefore);
                                    if hits >= req.max_hits {
                                        return Err("capture hit bound reached with other threads still stepping; missing completion is not success".into());
                                    }
                                    let site = sites[i];
                                    let thread_lifetime = capture_threads.lifetimes.get(&tid).copied().ok_or("capture thread lifetime missing")?;
                                    let site_base = site.image.base(exe.base, capture_core_base)?;
                                    instruction(&d.game, site_base, site)?;
                                    let stack_return = read_pointer(&d.game, c.Rsp).ok();
                                    let expected = plan::successor_with_return(site, site_base, c.Rcx, stack_return)?;
                                    let expected_base = if matches!(site.next, plan::Next::Rcx) {
                                        let core =
                                            module_named(req.pid, "GameCore_XP2_FinalRelease.dll")?;
                                        if !same_path(&core.path, &core_path)
                                            || sha256(&core.path)? != plan::CORE_SHA
                                            || c.Rcx != core.base + 0x752d50
                                            || read_exact::<5>(&d.game,c.Rcx)? != [0xe9,0x0b,0xcf,0xe3,0xff]
                                        {
                                            return Err(
                                                "factory target differs from pinned export".into(),
                                            );
                                        }
                                        core.base
                                    } else if matches!(site.next, plan::Next::StackReturn) {
                                        let core = initial_core.as_ref().ok_or("native return Core missing")?;
                                        if expected >= core.base && expected < core.base.checked_add(core.size).ok_or("Core image range overflow")? {
                                            core.base
                                        } else {
                                            exe.base
                                        }
                                    } else {
                                        site_base
                                    };
                                    executable(&d.game, expected_base, expected, 1)?;
                                    let pointer = match site.name {
                                        "context_release" | "context_replace" => {
                                            Some(read_pointer(
                                                &d.game,
                                                c.Rcx
                                                    .checked_add(0x450)
                                                    .ok_or("receiver overflow")?,
                                            )?)
                                        }
                                        "context_factory_return" | "lua_newstate_return" => {
                                            Some(c.Rax)
                                        }
                                        "lua_close_call" | "lua_close_deleting_call" => Some(c.Rcx),
                                        _ => None,
                                    };
                                    hits += 1;
                                    let context_slot = if matches!(site.name,"context_release"|"context_replace") {
                                        context_facts::capture(&d,&exe,site,&c,pointer.unwrap_or(0))
                                    } else {Value::Null};
                                    let shutdown = if phase == Phase::NativeShutdown {
                                        context_facts::shutdown(&d.game, site, &c)
                                    } else {Value::Null};
                                    let row = json!({"type":"instruction_before","capture_seq":hits,"tid":tid,"thread_lifetime":thread_lifetime,"site":site.name,"rva":site.rva,"phase":phase.name(),"rax":c.Rax,"rbx":c.Rbx,"rcx":c.Rcx,"rdx":c.Rdx,"r8":c.R8,"r9":c.R9,"rsp":c.Rsp,"rip":c.Rip,"stack_return":stack_return,"pointer_candidate":pointer,"context_slot":context_slot,"shutdown":shutdown,"gameplay_owner":"unknown","original_instruction_executions":null});
                                    mark(&mut d, row.clone())?;
                                    steps.insert(
                                        tid,
                                        Step {
                                            site,
                                            expected,
                                            armed_eflags: 0,
                                            capture: row,
                                            phase,
                                        },
                                    );
                                    c.EFlags |= TF | RF;
                                    c.Dr6 &= !((1 << 14) | 15);
                                    set_context(&d.threads[&tid].handle, &c)?;
                                    let armed = context(&d.threads[&tid].handle)?;
                                    if armed.EFlags & TF == 0 || armed.Rip != c.Rip || !installed_sites_match_context(&d.threads[&tid].armed_sites, &armed) {
                                        return Err("original instruction TF arming was not confirmed".into());
                                    }
                                    steps.get_mut(&tid).unwrap().armed_eflags = armed.EFlags;
                                }
                            }
                        }
                    } else if info.ExceptionRecord.ExceptionCode == THREAD_NAME_EXCEPTION
                        && d.cleanup_event.as_ref().is_some_and(|(_,s)| *s == DBG_CONTINUE) {
                        status = DBG_CONTINUE;
                    } else {
                        if steps.contains_key(&tid) {
                            d.last_dispatch =
                                DispatchOutcome::Ambiguous("exception during original instruction");
                            return Err("exception during original instruction".into());
                        }
                        status = DBG_EXCEPTION_NOT_HANDLED;
                    }
                }
                _ => {}
            }
            d.continue_pending_with_status(status)?;
            if announce_ready {
                emit(&json!({"ready":true,"pid":req.pid,"phase":phase.name(),"capture_only":true}))?;
            }
        }
        Ok(())
    })).unwrap_or_else(|_|Err("capture loop panicked; restoring before exit".into()));
    let mut stop_register_audit = Value::Null;
    let mut audit_write = Ok(());
    // Record before clearing TF or restoring DRs. Diagnostic I/O failure must
    // not bypass restoration or turn a verified detach into game termination.
    let cleanup = (|| -> Result<(), String> {
        let frozen = d.freeze();
        let audit = match &frozen {
            Ok(()) => stop_register_evidence(&d, epoch, &capture_threads, &steps),
            Err(e) => {
                json!({"type":"pre_restore_registers","summary":{"snapshot_status":"freeze_failed","tracked":d.threads.len(),"coverage_over_interval":"unknown","zero_hits_proves_unreached":false},"freeze_error":e})
            }
        };
        stop_register_audit = audit["summary"].clone();
        audit_write = mark(&mut d, audit);
        frozen?;
        // Clear ONLY the TF introduced by this adapter before shared restoration.
        for tid in steps.keys() {
            let state = d.threads.get(tid).ok_or("pending step lost thread")?;
            if unsafe { WaitForSingleObject(state.handle.0, 0) } == WAIT_OBJECT_0 {
                continue;
            }
            let mut c = context(&state.handle)?;
            c.EFlags &= !TF;
            set_context(&state.handle, &c)?;
        }
        if let (Some((event, _)), DispatchOutcome::Ambiguous(reason)) =
            (d.cleanup_event, d.last_dispatch)
        {
            d.critical_ambiguous_exception(&event, reason)
        } else {
            cleanup_order(&mut d)
        }
    })();
    let cleanup = match cleanup {
        Ok(()) => Ok(()),
        Err(e) if !d.critical => d.critical_wait_for_game_close(e),
        Err(e) => Err(e),
    };
    if cleanup.is_err() && d.critical && d.attached {
        // The existing critical path exhausted its bounded attempt. Retain the
        // debugger, pending event/thread handles and source pin until OS exit;
        // never publish order_stop or drop them merely because time elapsed.
        if unsafe { WaitForSingleObject(d.game.0, u32::MAX) } != WAIT_OBJECT_0 {
            plan::protocol::hold_recovery_handles("observer critical target wait failed");
        }
        d.attached = false;
        d.terminal = TerminalKind::TargetExited;
        d.held.clear();
    }
    let rows:Vec<_>=plan::inventory_for(true, true).map(|s|json!({"site":s.name,"completed":counts.get(s.name).copied().unwrap_or(0),"armed_windows":armed_windows.get(s.name).copied().unwrap_or(0),"coverage":"bounded_phase_only","missing_is_not_zero_calls":true})).collect();
    let terminal = d.terminal.label();
    let handles = d.threads.len();
    d.threads.clear();
    drop(source_pin);
    drop(input_reader);
    mark(
        &mut d,
        json!({"type":"order_stop","schema":plan::SCHEMA,"game_pid":req.pid,"game_created_filetime":req.created,"result":if result.is_ok()&&cleanup.is_ok()&&audit_write.is_ok(){"ok"}else{"error"},"error":result.as_ref().err(),"cleanup_error":cleanup.as_ref().err(),"stop_register_audit":stop_register_audit,"stop_register_record_error":audit_write.as_ref().err(),"terminal_kind":terminal,"debug_registers_restored":cleanup.is_ok(),"pending_instruction_completions":steps.len(),"sites":rows,"owned_thread_handles_closed":handles,"capture_thread_events":{"armed_creates":capture_threads.created,"exit_events":capture_threads.exited,"final_tracked":capture_threads.generations.len()},"phase_generation":epoch.generation,"input_reader_joined":true,"helper_processes_spawned":0,"controller_must_reap_child":true,"common_boundary_proof":"unknown","gameplay_lua_identity":"unknown","memory_writes":0,"dll_install_calls":0}),
    )?;
    result.and(cleanup).and(audit_write)
}

/// Parent stub: only invoke from the existing live_chain-owned controller lane.
/// Keeps Child + pinned game HANDLE through restoration/critical close, forwards
/// phase acknowledgements, and emits a separate real child-reaping receipt.
struct NativeRecovery {
    game: Handle,
    chain: OrderChain,
    req: Request,
    termination_requested: bool,
}
impl plan::protocol::Recovery for NativeRecovery {
    fn authorize(&mut self) -> Result<(), String> {
        verify_order_lock(
            Path::new("D:/CC/Tools/civ6-live/civ6.lock"),
            self.req.chain_pid,
            &self.chain,
        )?;
        if !alive(&self.game)?
            || process_created(&self.game)? != self.req.created
            || !is_pinned_dx11_exe(&process_path(&self.game)?)
        {
            return Err("pinned game changed before GO".into());
        }
        Ok(())
    }
    fn request_close(&mut self) -> Result<bool, String> {
        if self.target_exited() {
            return Ok(self.termination_requested);
        }
        if !self.termination_requested {
            if unsafe {
                windows_sys::Win32::System::Threading::TerminateProcess(self.game.0, 0x880)
            } == 0
            {
                return Err(win_error("TerminateProcess(pinned recovery)"));
            }
            self.termination_requested = true;
        }
        Ok(true)
    }
    fn target_exited(&mut self) -> bool {
        (unsafe { WaitForSingleObject(self.game.0, 0) }) == WAIT_OBJECT_0
    }
    fn wait_target_exit(&mut self) {
        // Existing native recovery handles termination acknowledgement. The
        // owning stack remains alive after its bounded attempt expires.
        let _ = announce_critical_and_wait_for_owned_game(
            &self.game,
            self.req.pid,
            "retained parent recovery",
            None,
            false,
            None,
            None,
            None,
            true,
        );
        if unsafe { WaitForSingleObject(self.game.0, u32::MAX) } != WAIT_OBJECT_0 {
            plan::protocol::hold_recovery_handles("target wait failed");
        }
    }
}

pub fn parent(mut value: Value) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use windows_sys::Win32::System::Threading::PROCESS_TERMINATE;
    let parent_pid_value = std::process::id();
    let self_handle = process(parent_pid_value, false)?;
    value["controller_pid"] = json!(parent_pid_value);
    value["controller_created_filetime"] = json!(process_created(&self_handle)?);
    let req = Request::parse(&value)?;
    let chain = OrderChain::pin(req.chain_pid, req.chain_created)?;
    verify_order_lock(
        Path::new("D:/CC/Tools/civ6-live/civ6.lock"),
        req.chain_pid,
        &chain,
    )?;
    chain_descendant(parent_pid_value, req.chain_pid)?;
    if req.created < req.chain_created
        || std::env::var("CIV6_LIVE_CHAIN_PID")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            != Some(req.chain_pid)
    {
        return Err("parent is outside the owning live_chain game lifetime".into());
    }
    let _pin = exact_process(
        req.pid,
        req.created,
        Path::new(crate::PINNED_DX11_EXE),
        false,
    )?;
    let game = Handle::new(
        unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE,
                0,
                req.pid,
            )
        },
        "OpenProcess(parent owned-game recovery)",
    )?;
    if process_created(&game)? != req.created || !is_pinned_dx11_exe(&process_path(&game)?) {
        return Err("parent game pin changed".into());
    }
    let manifest = req.output.with_extension("request.json");
    let mut file =
        File::create_new(&manifest).map_err(|e| format!("new manifest required: {e}"))?;
    serde_json::to_writer_pretty(&mut file, &value).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    let token = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{}:{}:{}:{}:{:?}",
                req.pid,
                req.created,
                req.controller_created,
                req.source_dll_sha,
                Instant::now()
            )
            .as_bytes()
        )
    );
    let child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .args([std::ffi::OsStr::new("observe"), manifest.as_os_str()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .env("TODO880_ATTACH_TOKEN", &token)
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| e.to_string())?;
    let witness = plan::protocol::Witness::Native {
        pid: req.pid,
        created: req.created,
        chain_pid: req.chain_pid,
        chain_created: req.chain_created,
        controller_pid: req.controller_pid,
        controller_created: req.controller_created,
        source_dll_sha: req.source_dll_sha.clone(),
        token,
    };
    let recovery = NativeRecovery {
        game,
        chain,
        req: req.clone(),
        termination_requested: false,
    };
    let mut guard =
        plan::protocol::ChildGuard::new(child, witness, Duration::from_secs(req.seconds), recovery);
    let (input_reader, input) = reader(Arc::new(AtomicBool::new(false)));
    let outcome = guard.run(&mut std::io::stdout(), &input);
    if outcome == plan::protocol::Outcome::CriticalRecovery {
        let _ = emit(
            &json!({"type":"critical_recovery_retained","bounded_api_expired":true,"cleanup_confirmed":false,"game_pid":req.pid}),
        );
        guard.resolve_critical();
    }
    let receipt = guard
        .receipt()
        .ok_or("recovery responsibility not discharged")?;
    drop(input_reader);
    let mut row = receipt.json();
    row["type"] = json!("order_parent_receipt");
    row["schema"] = json!(plan::SCHEMA);
    row["game_pid"] = json!(req.pid);
    row["game_created_filetime"] = json!(req.created);
    row["source_dll_sha256"] = json!(req.source_dll_sha);
    row["live_receipt"] = json!(receipt.restored);
    row["native_order_observed"] = json!(false);
    row["console_lease_generation_verified"] = json!(false);
    emit(&row)?;
    if receipt.cleanup_confirmed && receipt.exit_code == Some(0) && receipt.error.is_none() {
        Ok(())
    } else {
        Err("order capture failed; consult explicit cleanup receipt".into())
    }
}

#[cfg(test)]
#[path = "order_step_tests.rs"]
mod step_tests;

#[cfg(test)]
#[path = "order_phase_tests.rs"]
mod phase_tests;

#[cfg(test)]
#[path = "order_thread_name_tests.rs"]
mod thread_name_tests;

#[cfg(test)]
mod tests {
    use super::super::test_support::{AttachReadyBarrier, Fixture};
    use super::*;
    #[test]
    fn order_capture_chain_allowlist_rejects_other_copies_and_hashes() {
        assert_eq!(
            approved_chain_image(Path::new(DEFAULT_CHAIN)).unwrap(),
            DEFAULT_CHAIN
        );
        assert_eq!(
            approved_chain_image(Path::new(r"d:\cc\tools\civ6-live\LIVE_CHAIN_PR86_AMD.EXE"))
                .unwrap(),
            AMD_CHAIN
        );
        for path in [
            "D:/CC/Work/pr86-boundary-resume-548/chain-close-548/live_chain.exe",
            "D:/CC/Work/live_chain_pr86_amd.exe",
            "D:/CC/Tools/civ6-live/live_chain_pr86_amd.exe.other",
            "live_chain_pr86_amd.exe",
        ] {
            assert!(
                approved_chain_image(Path::new(path)).is_err(),
                "accepted {path}"
            );
            assert!(pin_chain_image(Path::new(path)).is_err());
        }
    }
    #[test]
    fn order_capture_chain_hash_and_file_sharing_use_owned_temp_file() {
        struct OwnedFile(PathBuf);
        impl Drop for OwnedFile {
            fn drop(&mut self) {
                let _ = fs::remove_file(&self.0);
            }
        }
        let path = std::env::temp_dir().join(format!(
            "todo880-chain-pin-{}-{}.bin",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut writer = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let owned = OwnedFile(path);
        writer.write_all(b"abc").unwrap();
        drop(writer);
        for hash in [
            "",
            AMD_CHAIN_SHA,
            "800b0cb71126412237428a956fd22fd68a98e7c4812879399235256de56b958e",
        ] {
            assert!(
                matches!(pin_hashed_chain_file(&owned.0, hash), Err(e) if e == "order observer AMD chain SHA mismatch")
            );
            // A refusal must release its handle as well.
            drop(fs::OpenOptions::new().write(true).open(&owned.0).unwrap());
        }
        for purpose in ["AMD chain", "Core"] {
            let pin = pin_hashed_image_file(
                &owned.0,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
                purpose,
            )
            .unwrap();
            assert_eq!(
                fs::OpenOptions::new()
                    .write(true)
                    .open(&owned.0)
                    .unwrap_err()
                    .raw_os_error(),
                Some(32)
            );
            assert_eq!(
                fs::remove_file(&owned.0).unwrap_err().raw_os_error(),
                Some(32)
            );
            assert_eq!(fs::read(&owned.0).unwrap(), b"abc");
            drop(pin);
            drop(fs::OpenOptions::new().write(true).open(&owned.0).unwrap());
        }
        fs::write(&owned.0, b"released").unwrap();
        fs::remove_file(&owned.0).unwrap();
        assert!(!owned.0.exists());
        // This policy check does not require an installed legacy executable.
        assert!(pin_chain_image(Path::new(DEFAULT_CHAIN)).unwrap().is_none());
    }
    #[test]
    fn order_capture_chain_wrong_filetime_is_rejected_before_image_admission() {
        let own = process(std::process::id(), false).unwrap();
        let created = process_created(&own).unwrap();
        assert!(
            matches!(OrderChain::pin(std::process::id(), created + 1), Err(e) if e == "order observer chain PID/FILETIME changed")
        );
        assert!(
            matches!(OrderChain::pin(std::process::id(), created), Err(e) if e == "order observer chain image is not approved")
        );
    }
    #[test]
    fn order_capture_owned_single_step_keeps_continue_when_output_fails() {
        let mut child = Fixture::start();
        let pid = child.child.id();
        let base = child.entry - CAN_EVER_RVA;
        let path = std::env::temp_dir().join(format!(
            "todo880-order-write-failure-{}-{pid}.jsonl",
            std::process::id()
        ));
        let output = Jsonl::new(&path).unwrap();
        let (stop_tx, _) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let observer = child.observe(&stop_tx, move |ack| {
            let mut d = Debugger::attach(
                pid,
                process(pid, true).unwrap(),
                base,
                0x500000,
                output,
                ProbeMode::Governor,
            )
            .unwrap();
            d.close_ack = ack;
            let mut ready_barrier = AttachReadyBarrier::default();
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                assert!(
                    Instant::now() < deadline,
                    "owned helper did not deliver event"
                );
                let mut event = DEBUG_EVENT::default();
                if unsafe { WaitForDebugEventEx(&mut event, 50) } == 0 {
                    continue;
                }
                d.cleanup_event = Some((event, DBG_EXCEPTION_NOT_HANDLED));
                if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT
                    && unsafe { event.u.Exception.ExceptionRecord.ExceptionCode }
                        == EXCEPTION_SINGLE_STEP
                {
                    d.last_dispatch = DispatchOutcome::Unrelated;
                    // Actual OS file-write refusal while our real Rust child is
                    // stopped on a real hardware event; no forged game identity.
                    let mut readonly = File::open(&path).unwrap();
                    assert!(classify_then_record(
                        &mut d,
                        &event,
                        &BTreeMap::new(),
                        &[base + CAN_EVER_RVA, base + CAN_APPOINT_RVA],
                        |d, _row| {
                            assert_eq!(d.status_after_error(&event), DBG_CONTINUE);
                            readonly.write_all(b"cannot write")
                        }
                    )
                    .is_err());
                    assert_eq!(d.status_after_error(&event), DBG_CONTINUE);
                    assert_eq!(d.cleanup_event.as_ref().unwrap().1, DBG_CONTINUE);
                    d.cleanup(Some((&event, DBG_CONTINUE))).unwrap();
                    assert_eq!(d.terminal, TerminalKind::Detached);
                    return path;
                }
                let status = d.handle_event(&event).unwrap();
                d.continue_pending_with_status(status).unwrap();
                if ready_barrier.advance(&mut d, &event) {
                    ready_tx.send(()).unwrap();
                }
            }
        });
        ready_rx.recv_timeout(Duration::from_secs(8)).unwrap();
        assert_eq!(child.command("T", "PROBE_OUT:"), "1");
        let path = observer.join().unwrap();
        assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
        let status = child.terminate();
        assert!(!status.success());
        println!(
            "TODO880_OS_RECEIPT={}",
            json!({"case":"owned_single_step_output_failure","kind":"owned_rust_debug_helper","helper_pid":pid,"owned_exception_continue":true,"debug_registers_restored":true,"child_explicitly_terminated":true,"child_reaped":true,"civ6_live":false,"game_launches":0})
        );
        drop(child);
        fs::remove_file(path).unwrap();
    }
}
