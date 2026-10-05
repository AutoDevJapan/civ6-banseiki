//! Capture-only adapter over the existing observer's attach/cleanup machinery.
use super::*;
use crate::order_capture::{self as plan, Phase, Request, Site};
use windows_sys::Win32::System::{
    Memory::{
        VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_IMAGE, PAGE_EXECUTE,
        PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_GUARD,
    },
    Pipes::PeekNamedPipe,
};

const TF: u32 = 1 << 8;
struct Step {
    site: Site,
    expected: u64,
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
fn arm(d: &mut Debugger, tid: u32, phase: Phase, req: &Request) -> Result<(), String> {
    if !d.threads.contains_key(&tid) {
        let handle = thread_handle(tid)?;
        if unsafe { GetProcessIdOfThread(handle.0) } != d.pid
            || unsafe { GetThreadId(handle.0) } != tid
        {
            return Err("unknown thread ownership".into());
        }
        let c = context(&handle)?;
        if c.Dr7 & 0xff != 0 || c.EFlags & TF != 0 {
            return Err("thread already has debugger state".into());
        }
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
    }
    let state = d.threads.get_mut(&tid).ok_or("thread not tracked")?;
    let mut c = context(&state.handle)?;
    if state.armed_sites.iter().any(Option::is_some)
        && !installed_sites_match_context(&state.armed_sites, &c)
    {
        return Err("prior capture debug-register ownership changed".into());
    }
    let sites = plan::prefix(phase, req.count(phase))?;
    let mut addresses = [0; 4];
    for (i, s) in sites.iter().enumerate() {
        addresses[i] = d.module_base + s.rva;
    }
    [c.Dr0, c.Dr1, c.Dr2, c.Dr3] = addresses;
    c.Dr7 =
        (c.Dr7 & !0xff & !(0xffff << 16)) | (0..sites.len()).map(|i| 1u64 << (i * 2)).sum::<u64>();
    c.Dr6 &= !0xf;
    state.armed_sites = addresses.map(|v| (v != 0).then_some(v));
    set_context(&state.handle, &c)?;
    if !installed_sites_match_context(&state.armed_sites, &context(&state.handle)?) {
        return Err("armed registers differ".into());
    }
    Ok(())
}
fn read_pointer(game: &Handle, address: u64) -> Result<u64, String> {
    Ok(u64::from_le_bytes(read_exact::<8>(game, address)?))
}
fn mark(d: &mut Debugger, mut row: Value) -> Result<(), String> {
    row["sequence"] = json!(d.output.rows + 1);
    d.stamp(&mut row);
    d.output.row(row.clone())?;
    if row["type"] == "order_stop" {
        writeln!(std::io::stdout(), "{row}").map_err(|e| e.to_string())?;
        std::io::stdout().flush().map_err(|e| e.to_string())?;
    }
    Ok(())
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
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_INPUT_HANDLE};
    let h = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    let mut available = 0;
    if unsafe {
        PeekNamedPipe(
            h,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    } == 0
    {
        if std::io::Error::last_os_error().raw_os_error() == Some(109) {
            return Ok(vec!["eof".into()]);
        }
        return Err("controller stdin must be a live pipe".into());
    }
    if available > 0 {
        let mut bytes = vec![0; available.min(1024) as usize];
        let n = std::io::stdin()
            .read(&mut bytes)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(vec!["eof".into()]);
        }
        buffer.extend_from_slice(&bytes[..n]);
    }
    if buffer.len() > 4096 {
        return Err("controller command exceeds limit".into());
    }
    let mut out = Vec::new();
    while let Some(n) = buffer.iter().position(|b| *b == b'\n') {
        let bytes: Vec<_> = buffer.drain(..=n).collect();
        out.push(
            std::str::from_utf8(&bytes)
                .map_err(|e| e.to_string())?
                .trim()
                .into(),
        );
    }
    Ok(out)
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

pub fn observe(req: Request) -> Result<(), String> {
    // Resolve authority before attach. No lock acquisition, reservation, launch,
    // injection, DLL loading, memory setter or runtime API is present here.
    let chain = exact_process(
        req.chain_pid,
        req.chain_created,
        Path::new("D:/CC/Tools/civ6-live/live_chain.exe"),
        false,
    )?;
    let lock = Path::new("D:/CC/Tools/civ6-live/civ6.lock");
    verify_lock(lock, req.chain_pid, &chain)?;
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
    let static_proof = plan::disk_sites(&disk)?;
    let core_path = Path::new(GAME_ROOT).join(MODULE_REL);
    if sha256(&core_path)? != plan::CORE_SHA {
        return Err("GameCore disk hash differs".into());
    }
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
    let game_guard = exact_process(req.pid, req.created, &exe.path, false)?;
    let output = Jsonl::new(&req.output)?;
    writeln!(std::io::stdout(),"{}",json!({"type":"order_attach_start","schema":plan::SCHEMA,"game_pid":req.pid,"game_created_filetime":req.created})).map_err(|e|e.to_string())?;
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    // Attach in existing non-hardware mode. Only this explicit capability then
    // arms the fixed EXE sites; old live hardware modes stay rejected.
    let mut d = Debugger::attach(
        req.pid,
        game,
        exe.base,
        exe.size,
        output,
        ProbeMode::Lifecycle,
    )?;
    d.mode = ProbeMode::Property; // existing restoration walks our pinned threads
    d.attach_ready_deadline = Some(Instant::now() + ATTACH_READY_BUDGET);
    let initial_core = core_module(req.pid)?;
    if initial_core
        .as_ref()
        .is_some_and(|m| !same_path(&m.path, &core_path))
    {
        return Err("initial GameCore path differs".into());
    }
    let initial_context = read_pointer(&d.game, exe.base + 0x22d60c0)?;
    mark(
        &mut d,
        json!({"type":"order_identity","schema":plan::SCHEMA,"game_pid":req.pid,"game_created_filetime":req.created,"chain_pid":req.chain_pid,"chain_created_filetime":req.chain_created,"controller_pid":req.controller_pid,"controller_created_filetime":req.controller_created,"source_dll":req.source_dll,"source_dll_sha256":req.source_dll_sha,"exe_base":exe.base,"static_proof":static_proof,"initial_core_base":initial_core.as_ref().map(|m|m.base),"initial_context":initial_context,"first_factory_since_process_start":"unknown","gameplay_lua_identity":"unknown","memory_writes":0,"dll_install_calls":0}),
    )?;
    let mut phase = req.phase;
    let mut desired = phase;
    let mut steps: BTreeMap<u32, Step> = BTreeMap::new();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let (input_reader, input) = reader(d.close_ack.clone());
    let started = Instant::now();
    let mut stop = false;
    let mut hits = 0;
    let mut drain_deadline = None;
    let mut phase_changes = 0;
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
                    "game_close_requested" => d.close_ack.store(true, Ordering::SeqCst),
                    _ => return Err("unknown command; use phase front, phase load, stop".into()),
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
            verify_lock(lock, req.chain_pid, &chain)?;
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
                    d.freeze()?;
                    // An old-phase trap may have arrived between timeout and
                    // suspension. Consume it under the OLD site set first.
                    if unsafe{WaitForDebugEventEx(&mut event,0)}!=0 {
                        d.cleanup_event=Some((event,DBG_EXCEPTION_NOT_HANDLED));
                        d.resume()?;
                        // Fall through to normal event handling without switching.
                    } else {
                        if std::io::Error::last_os_error().raw_os_error()!=Some(121){return Err(win_error("phase drain WaitForDebugEventEx"));}
                        phase_changes += 1;
                        if phase_changes > 8 {
                            return Err("phase change bound exceeded".into());
                        }
                        for s in plan::prefix(desired, req.count(desired))? {
                            instruction(&d.game, exe.base, *s)?;
                        }
                        let tids: Vec<_> = d.threads.keys().copied().collect();
                        for t in tids {
                            arm(&mut d, t, desired, &req)?;
                        }
                        mark(
                            &mut d,
                            json!({"type":"phase_gap","old":phase.name(),"new":desired.name(),"unarmed_sites":"not_observed","first_factory_since_process_start":"unknown"}),
                        )?;
                        phase = desired;
                        for s in plan::prefix(phase, req.count(phase))? {
                            *armed_windows.entry(s.name.into()).or_default() += 1;
                        }
                        d.resume()?;
                        println!("{}", json!({"phase_ready":phase.name(),"pid":req.pid}));
                        std::io::stdout().flush().map_err(|e| e.to_string())?;
                        continue;
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
                CREATE_PROCESS_DEBUG_EVENT => {
                    let info = unsafe { event.u.CreateProcessInfo };
                    if !info.hFile.is_null() {
                        unsafe {
                            CloseHandle(info.hFile);
                        }
                    }
                    // Windows owns hProcess/hThread in debug events until exit continuation.
                    arm(&mut d, tid, phase, &req)?;
                }
                CREATE_THREAD_DEBUG_EVENT => {
                    let start =
                        unsafe { event.u.CreateThread.lpStartAddress }.map(|p| p as usize as u64);
                    d.attach_origin.note_thread_start(tid, start);
                    arm(&mut d, tid, phase, &req)?;
                }
                EXIT_THREAD_DEBUG_EVENT => {
                    if steps.remove(&tid).is_some() {
                        return Err("thread exited before original instruction completion".into());
                    }
                    d.attach_origin.note_thread_exit(tid);
                    d.threads.remove(&tid);
                }
                LOAD_DLL_DEBUG_EVENT => {
                    let info = unsafe { event.u.LoadDll };
                    if !info.hFile.is_null() {
                        unsafe {
                            CloseHandle(info.hFile);
                        }
                    }
                    let attach_snapshot = !d.attach_origin.accepted;
                    mark(
                        &mut d,
                        json!({"type":if attach_snapshot {"module_attach_snapshot"}else{"module_load"},"tid":tid,"base":info.lpBaseOfDll as u64,"source":"real_debug_event","load_since_ready":!attach_snapshot}),
                    )?;
                    if let Some(core) = core_module(req.pid)? {
                        if !same_path(&core.path, &core_path)
                            || sha256(&core.path)? != plan::CORE_SHA
                        {
                            return Err("loaded GameCore identity differs".into());
                        }
                    }
                }
                UNLOAD_DLL_DEBUG_EVENT => {
                    mark(
                        &mut d,
                        json!({"type":"module_unload","tid":tid,"base":unsafe {event.u.UnloadDll.lpBaseOfDll} as u64,"source":"real_debug_event"}),
                    )?;
                }
                EXIT_PROCESS_DEBUG_EVENT => {
                    d.continue_pending_with_status(DBG_CONTINUE)?;
                    return Err("game exited during capture".into());
                }
                EXCEPTION_DEBUG_EVENT => {
                    let info = unsafe { event.u.Exception };
                    let address = info.ExceptionRecord.ExceptionAddress as u64;
                    mark(&mut d,json!({"type":"debug_exception","tid":tid,"code":info.ExceptionRecord.ExceptionCode,"address":address,"first_chance":info.dwFirstChance!=0,"phase":phase.name()}))?;
                    if info.ExceptionRecord.ExceptionCode == EXCEPTION_BREAKPOINT {
                        if steps.contains_key(&tid) {
                            d.last_dispatch = DispatchOutcome::Ambiguous(
                                "breakpoint interrupted original instruction",
                            );
                            return Err("breakpoint interrupted original instruction".into());
                        }
                        let observed = inspect_thread_origin(req.pid, tid, d.query_thread_start);
                        let owned = d.classify_attach_breakpoint(
                            tid,
                            address,
                            info.dwFirstChance != 0,
                            observed,
                        );
                        d.last_attach_breakpoint_owned = owned;
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
                            plan::verify_step(step.expected, address, c.Rip, c.Dr6,c.EFlags)?;
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
                            row["original_instruction_executions"] = json!(1);
                            row["phase"] = json!(step.phase.name());
                            *counts.entry(step.site.name.into()).or_default() += 1;
                            mark(&mut d, row)?;
                        } else {
                            let sites = plan::prefix(phase, req.count(phase))?;
                            let addresses: Vec<_> =
                                sites.iter().map(|s| exe.base + s.rva).collect();
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
                                    instruction(&d.game, exe.base, site)?;
                                    let expected = plan::successor(site, exe.base, c.Rcx)?;
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
                                    } else {
                                        exe.base
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
                                    let row = json!({"type":"instruction_before","capture_seq":hits,"tid":tid,"site":site.name,"rva":site.rva,"phase":phase.name(),"rax":c.Rax,"rbx":c.Rbx,"rcx":c.Rcx,"rdx":c.Rdx,"r8":c.R8,"r9":c.R9,"rsp":c.Rsp,"rip":c.Rip,"pointer_candidate":pointer,"gameplay_owner":"unknown","original_instruction_executions":null});
                                    mark(&mut d, row.clone())?;
                                    steps.insert(
                                        tid,
                                        Step {
                                            site,
                                            expected,
                                            capture: row,
                                            phase,
                                        },
                                    );
                                    c.EFlags |= TF | RF;
                                    c.Dr6 &= !((1 << 14) | 15);
                                    set_context(&d.threads[&tid].handle, &c)?;
                                }
                            }
                        }
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
                println!("{}",json!({"ready":true,"pid":req.pid,"phase":phase.name(),"capture_only":true}));
                std::io::stdout().flush().map_err(|e|e.to_string())?;
            }
        }
        Ok(())
    })).unwrap_or_else(|_|Err("capture loop panicked; restoring before exit".into()));
    // Clear ONLY the TF introduced by this adapter before shared restoration.
    let cleanup = (|| -> Result<(), String> {
        d.freeze()?;
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
            d.cleanup(None)
        }
    })();
    let cleanup = match cleanup {
        Ok(()) => Ok(()),
        Err(e) if !d.critical => d.critical_wait_for_game_close(e),
        Err(e) => Err(e),
    };
    let rows:Vec<_>=plan::FRONT.iter().chain(plan::LOAD.iter()).map(|s|json!({"site":s.name,"completed":counts.get(s.name).copied().unwrap_or(0),"armed_windows":armed_windows.get(s.name).copied().unwrap_or(0),"coverage":"bounded_phase_only","missing_is_not_zero_calls":true})).collect();
    let terminal = d.terminal.label();
    let handles = d.threads.len();
    d.threads.clear();
    drop(source_pin);
    drop(input_reader);
    mark(
        &mut d,
        json!({"type":"order_stop","schema":plan::SCHEMA,"game_pid":req.pid,"game_created_filetime":req.created,"result":if result.is_ok()&&cleanup.is_ok(){"ok"}else{"error"},"error":result.as_ref().err(),"cleanup_error":cleanup.as_ref().err(),"terminal_kind":terminal,"debug_registers_restored":cleanup.is_ok(),"pending_instruction_completions":steps.len(),"sites":rows,"owned_thread_handles_closed":handles,"input_reader_joined":true,"helper_processes_spawned":0,"controller_must_reap_child":true,"common_boundary_proof":"unknown","gameplay_lua_identity":"unknown","memory_writes":0,"dll_install_calls":0}),
    )?;
    result.and(cleanup)
}

/// Parent stub: only invoke from the existing live_chain-owned controller lane.
/// Keeps Child + pinned game HANDLE through restoration/critical close, forwards
/// phase acknowledgements, and emits a separate real child-reaping receipt.
pub fn parent(mut value: Value) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use windows_sys::Win32::System::Threading::{TerminateProcess, PROCESS_TERMINATE};
    let parent_pid_value = std::process::id();
    let self_handle = process(parent_pid_value, false)?;
    value["controller_pid"] = json!(parent_pid_value);
    value["controller_created_filetime"] = json!(process_created(&self_handle)?);
    let req = Request::parse(&value)?;
    let chain = exact_process(
        req.chain_pid,
        req.chain_created,
        Path::new("D:/CC/Tools/civ6-live/live_chain.exe"),
        false,
    )?;
    verify_lock(
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
    let mut child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .args([std::ffi::OsStr::new("observe"), manifest.as_os_str()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| e.to_string())?;
    let child_pid = child.id();
    let mut child_in = child.stdin.take();
    let out = child.stdout.take().ok_or("child stdout missing")?;
    let (tx, rx) = mpsc::channel();
    let output_reader = thread::spawn(move || {
        for line in std::io::BufReader::new(out).lines() {
            if tx.send(line.map_err(|e| e.to_string())).is_err() {
                break;
            }
        }
    });
    let (input_reader, input) = reader(Arc::new(AtomicBool::new(false)));
    let deadline = Instant::now() + Duration::from_secs(req.seconds + 40);
    let mut recovery_deadline = None;
    let mut close_requested = false;
    let mut parent_error = None;
    let mut stop_restored = false;
    let mut attach_attempted = false;
    let mut child_exit = None;
    let mut output_closed = false;
    let status = loop {
        loop {
            let line = match rx.try_recv() {
                Ok(line) => line,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    output_closed = true;
                    break;
                }
            };
            match line {
                Ok(line) => {
                    println!("{line}");
                    let _ = std::io::stdout().flush();
                    if serde_json::from_str::<Value>(&line).ok().is_some_and(|v| {
                        v["critical_cleanup"] == true
                            && v["pid"].as_u64() == Some(u64::from(req.pid))
                    }) {
                        parent_error = Some("observer critical cleanup".to_string());
                    }
                    if let Ok(v) = serde_json::from_str::<Value>(&line) {
                        if v["type"] == "order_attach_start"
                            && v["schema"] == plan::SCHEMA
                            && v["game_pid"].as_u64() == Some(u64::from(req.pid))
                            && v["game_created_filetime"].as_u64() == Some(req.created)
                        {
                            attach_attempted = true;
                        }
                        stop_restored |= plan::restored_stop(&v, req.pid, req.created);
                    }
                }
                Err(e) => {
                    parent_error = Some(e);
                }
            }
        }
        for cmd in input.try_iter() {
            match cmd {
                Ok(cmd) if matches!(cmd.as_str(), "phase front" | "phase load" | "stop") => {
                    if let Some(pipe) = child_in.as_mut() {
                        if writeln!(pipe, "{cmd}").and_then(|_| pipe.flush()).is_err() {
                            parent_error = Some("child command pipe failed".into());
                        }
                    }
                }
                _ => {
                    parent_error = Some("parent command rejected".into());
                }
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                child_exit = Some(status);
            }
            Ok(None) => {}
            Err(e) => {
                parent_error = Some(e.to_string());
            }
        }
        if let Some(status) = child_exit {
            if output_closed {
                if !attach_attempted && parent_error.is_none() {
                    break status;
                }
                if stop_restored && parent_error.is_none() {
                    break status;
                }
                parent_error
                    .get_or_insert("child exited without verified restoration receipt".into());
                if unsafe { WaitForSingleObject(game.0, 0) } == WAIT_OBJECT_0 {
                    break status;
                }
            }
        }
        if Instant::now() >= deadline {
            parent_error.get_or_insert("observer exceeded parent deadline".into());
        }
        if parent_error.is_some() && !close_requested {
            // The original process HANDLE, not a fresh PID lookup, is authority.
            if unsafe { WaitForSingleObject(game.0, 0) } == WAIT_OBJECT_0
                || unsafe { TerminateProcess(game.0, 0x880) } != 0
            {
                close_requested = true;
                if let Some(pipe) = child_in.as_mut() {
                    let _ = writeln!(pipe, "game_close_requested");
                    let _ = pipe.flush();
                }
                recovery_deadline = Some(Instant::now() + Duration::from_secs(15));
            } else {
                parent_error = Some(win_error("TerminateProcess(pinned owned game)"));
                recovery_deadline.get_or_insert(Instant::now() + Duration::from_secs(15));
            }
        }
        if recovery_deadline.is_some_and(|end| Instant::now() >= end) {
            if unsafe { WaitForSingleObject(game.0, 0) } == WAIT_OBJECT_0 {
                let _ = child.kill();
                break child.wait().map_err(|e| e.to_string())?;
            }
            // A live debuggee must not be abandoned with a forcibly killed
            // debugger. The controller stays here retaining both handles.
            // Report this distinctly; it is not a successful bounded capture.
            println!(
                "{}",
                json!({"critical_cleanup":true,"pid":req.pid,"child_pid":child_pid,"game_close_required":true,"parent_retaining_unresolved_handles":true})
            );
            recovery_deadline = Some(Instant::now() + Duration::from_secs(15));
        }
        thread::sleep(Duration::from_millis(25));
    };
    drop(child_in.take());
    drop(input_reader);
    let output_joined = output_reader.join().is_ok();
    println!(
        "{}",
        json!({"type":"order_parent_receipt","schema":plan::SCHEMA,"child_pid":child_pid,"child_reaped":true,"child_exit_code":status.code(),"stdout_reader_joined":output_joined,"stdin_reader_joined":true,"attach_attempted":attach_attempted,"preflight_refused":!attach_attempted&&!status.success(),"owned_game_close_requested":close_requested,"game_exit_observed":unsafe{WaitForSingleObject(game.0,0)}==WAIT_OBJECT_0,"parent_error":parent_error,"game_pid":req.pid,"game_created_filetime":req.created,"source_dll_sha256":req.source_dll_sha,"live_receipt":true})
    );
    if status.success() && stop_restored && parent_error.is_none() && output_joined {
        Ok(())
    } else {
        Err("order capture/reaping receipt reports failure".into())
    }
}
