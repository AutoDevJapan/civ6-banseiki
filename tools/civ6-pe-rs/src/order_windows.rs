//! Capture-only adapter over the existing observer's attach/cleanup machinery.
use super::*;
use crate::order_capture::{self as plan, Phase, Request, Site};
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
fn verify_amd_chain_hash(actual: &str) -> Result<(), String> {
    if actual != AMD_CHAIN_SHA {
        return Err("order observer AMD chain SHA mismatch".into());
    }
    Ok(())
}
fn pin_chain_image(path: &Path) -> Result<Option<File>, String> {
    use std::os::windows::fs::OpenOptionsExt;
    let image = approved_chain_image(path)?;
    if image == DEFAULT_CHAIN {
        return Ok(None);
    }
    // Keep the approved on-disk image immutable through GO, observation and
    // recovery. Hash once while denying writes/deletes, not on every event.
    let pin = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(image)
        .map_err(|e| format!("AMD chain image pin failed: {e}"))?;
    verify_amd_chain_hash(&sha256(Path::new(image))?)?;
    Ok(Some(pin))
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

fn qualify_exception(
    d: &mut Debugger,
    event: &DEBUG_EVENT,
    steps: &BTreeMap<u32, Step>,
    sites: &[u64],
) -> Result<(), String> {
    if event.dwDebugEventCode != EXCEPTION_DEBUG_EVENT {
        return Ok(());
    }
    let info = unsafe { event.u.Exception };
    let tid = event.dwThreadId;
    let address = info.ExceptionRecord.ExceptionAddress as u64;
    match info.ExceptionRecord.ExceptionCode {
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
                plan::verify_step(step.expected, address, c.Rip, c.Dr6, c.EFlags)?;
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
    Ok(())
}

fn classify_then_record(
    d: &mut Debugger,
    event: &DEBUG_EVENT,
    steps: &BTreeMap<u32, Step>,
    sites: &[u64],
    record: impl FnOnce(&mut Debugger) -> std::io::Result<()>,
) -> Result<(), String> {
    plan::protocol::classify_then_record(
        d,
        |d| {
            qualify_exception(d, event, steps, sites)?;
            Ok(d.status_after_error(event))
        },
        record,
    )
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

pub fn observe(req: Request) -> Result<(), String> {
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
        json!({"type":"order_identity","schema":plan::SCHEMA,"game_pid":req.pid,"game_created_filetime":req.created,"chain_pid":req.chain_pid,"chain_created_filetime":req.chain_created,"controller_pid":req.controller_pid,"controller_created_filetime":req.controller_created,"source_dll":req.source_dll,"source_dll_sha256":req.source_dll_sha,"exe_base":exe.base,"capture_phase":req.phase.name(),"front_prefix":req.front_prefix,"load_prefix":req.load_prefix,"static_proof":static_proof,"initial_core_base":initial_core.as_ref().map(|m|m.base),"initial_context":initial_context,"first_factory_since_process_start":"unknown","gameplay_lua_identity":"unknown","memory_writes":0,"dll_install_calls":0}),
    )?;
    let mut phase = req.phase;
    let mut desired = phase;
    let mut steps: BTreeMap<u32, Step> = BTreeMap::new();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let (input_reader, input) = reader(d.close_ack.clone());
    let started = Instant::now();
    let mut stop = admission.stop_requested();
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
                        emit(&json!({"phase_ready":phase.name(),"pid":req.pid}))?;
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
                    let row=json!({"type":"debug_exception","tid":tid,"code":info.ExceptionRecord.ExceptionCode,"address":address,"first_chance":info.dwFirstChance!=0,"phase":phase.name()});
                    let sites:Vec<_>=plan::prefix(phase,req.count(phase))?.iter().map(|s|exe.base+s.rva).collect();
                    classify_then_record(&mut d,&event,&steps,&sites,|d|mark(d,row).map_err(std::io::Error::other))?;
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
                emit(&json!({"ready":true,"pid":req.pid,"phase":phase.name(),"capture_only":true}))?;
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
mod tests {
    use super::super::test_support::Fixture;
    use super::*;
    #[test]
    fn order_capture_chain_allowlist_rejects_other_copies_and_hashes() {
        assert_eq!(
            approved_chain_image(Path::new(DEFAULT_CHAIN)).unwrap(),
            DEFAULT_CHAIN
        );
        assert_eq!(
            approved_chain_image(Path::new(r"d:\cc\tools\civ6-live\LIVE_CHAIN_PR86_AMD.EXE")).unwrap(),
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
        for hash in [
            "",
            "800b0cb71126412237428a956fd22fd68a98e7c4812879399235256de56b958e",
            "8a4d2d307b0a8f0cf3f2156311d9673d9490c99bafff3cbddbc1fe4e34a0a7ac",
        ] {
            assert!(verify_amd_chain_hash(hash).is_err());
        }
    }
    #[test]
    fn order_capture_chain_actual_disk_hash_is_pinned_without_launch() {
        let pin = pin_chain_image(Path::new(AMD_CHAIN)).unwrap();
        assert!(pin.is_some());
        // The legacy path retains its existing path-only policy.
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
                let status;
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
                        |d| {
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
                status = d.handle_event(&event).unwrap();
                d.continue_pending_with_status(status).unwrap();
                if d.ready {
                    d.ready = false;
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
