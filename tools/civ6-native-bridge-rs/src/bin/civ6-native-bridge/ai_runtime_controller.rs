//! Commands on the already-owned resident controller. Never loads another DLL.
use super::*;
use civ6_native_bridge_rs::{
    AI_OBSERVATION_VERSION, AiObservationConfig, AiRuntimeState,
    ai_diagnostic::{Command, Session},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cleanup {
    Absent,
    Pending(Command),
    StopUnconfirmed,
    Stopped,
}
pub(super) struct Control {
    session: Session,
    writer: Option<BufWriter<File>>,
    cleanup: Cleanup,
}
impl Control {
    pub(super) fn new() -> Self {
        Self {
            session: Session::default(),
            writer: None,
            cleanup: Cleanup::Absent,
        }
    }
    pub(super) fn handle(
        &mut self,
        value: &serde_json::Value,
        parsed: &Args,
        game: &Handle,
        image: BridgeImage,
        dll: &Path,
    ) -> Result<serde_json::Value, String> {
        let command = Command::parse(value, parsed.pid, parsed.created)?;
        self.session.reserve(command)?;
        // Reserve and create the artifact BEFORE any native registration.
        if self.writer.is_none() {
            self.writer = Some(BufWriter::new(
                File::create_new(parsed.out.with_extension("ai.jsonl"))
                    .map_err(|e| e.to_string())?,
            ));
        }
        let request = serde_json::json!({"kind":"ai_request","request":value,"pid":parsed.pid,
            "created_filetime":parsed.created,"execution_mode":0,"execution_blockers":7});
        write_json(self.writer.as_mut().unwrap(), request)?;
        let result = self.call(command, game, image, dll);
        let receipt = match result {
            Ok(v) => v,
            Err(e) => serde_json::json!({"kind":"ai_observation","request":value,"error":e,
                "pid":parsed.pid,"created_filetime":parsed.created,"outcome":"unknown_or_refused",
                "retry_allowed":false,"execution_blockers":7,"operation_proven":false}),
        };
        write_json(self.writer.as_mut().unwrap(), receipt.clone())?;
        Ok(receipt)
    }
    fn call(
        &mut self,
        command: Command,
        game: &Handle,
        image: BridgeImage,
        dll: &Path,
    ) -> Result<serde_json::Value, String> {
        let local = LocalDll(unsafe {
            LoadLibraryExW(
                wide(&dll.to_string_lossy()).as_ptr(),
                null_mut(),
                DONT_RESOLVE_DLL_REFERENCES,
            )
        });
        if local.0.is_null() {
            return Err("AI export inspection failed".into());
        }
        let resolve = |name: &std::ffi::CStr| -> Result<usize, String> {
            let address = unsafe { GetProcAddress(local.0, name.as_ptr().cast()) }
                .ok_or("AI export missing")? as usize;
            let rva = address
                .checked_sub(local.0 as usize)
                .filter(|r| *r < image.size)
                .ok_or("AI export outside DLL")?;
            image
                .base
                .checked_add(rva)
                .ok_or("AI export overflow".into())
        };
        let query = resolve(c"BridgeRuntimeAiCapture")?;
        let action = resolve(if command.stop {
            c"BridgeRuntimeAiStopObservation"
        } else {
            c"BridgeRuntimeAiObserve"
        })?;
        let s = command.subject;
        let mut state: AiRuntimeState = unsafe { std::mem::zeroed() };
        state.size = size_of::<AiRuntimeState>() as u32;
        state.version = AI_OBSERVATION_VERSION;
        state.game_pid = s.pid;
        state.created_filetime = s.created;
        state.owner = s.owner;
        state.generation = s.generation;
        state.governor_index = s.index;
        let mut config = AiObservationConfig {
            size: size_of::<AiObservationConfig>() as u32,
            version: AI_OBSERVATION_VERSION,
            game_pid: s.pid,
            created_filetime: s.created,
            owner: s.owner,
            generation: s.generation,
            governor_index: s.index,
            status: 0,
            execution_mode: 0,
            expected_cost: s.cost,
            expected_paid: s.paid,
            expected_spent: s.spent,
            expected_title: s.title,
        };
        let remote = if command.stop {
            prepare_struct(game, &mut state)?
        } else {
            prepare_struct(game, &mut config)?
        };
        // Everything above is preflight. Only crossing the native-call boundary
        // creates a possible installed registration and a pending cleanup.
        let status = self.invoke_registration(game, action, remote, command)?;
        let mut capture: Box<civ6_native_bridge_rs::AiCapture> =
            Box::new(unsafe { std::mem::zeroed() });
        capture.state = state;
        let query_status = call_struct(game, query, &mut *capture)?;
        if capture.count > 128 {
            return Err("AI capture count exceeds fixed buffer".into());
        }
        let rows:Vec<_>=capture.rows[..capture.count as usize].iter().map(|r|serde_json::json!({
            "sequence":r.sequence,"thread":r.thread,"flags":r.flags,"owner":r.owner,"generation":r.generation,
            "index":r.index,"own_owner":r.own_owner,"promotion_count":r.promotion_count,"owned":r.owned.as_slice(),
            "site":r.raw.site,"after":r.flags&1!=0,"accepted":r.flags&2!=0,"capture_entry":r.flags&4!=0,
            "rcx":r.raw.rcx,"rdx":r.raw.rdx,"r8":r.raw.r8,"r9":r.raw.r9,"rax":r.raw.rax,
            "r10":r.raw.r10,"r11":r.raw.r11,"rflags":r.raw.flags,"xmm":r.raw.xmm,
            "caller_rsp":r.raw.caller_rsp,"return_address":r.raw.return_address,"ticket":r.raw.ticket
        })).collect();
        let state = capture.state;
        let receipt = serde_json::json!({"kind":"ai_observation","stop":command.stop,"pid":s.pid,
            "created_filetime":s.created,"owner":s.owner,"index":s.index,"requested_generation":s.generation,
            "cost":s.cost,"paid":s.paid,"spent":s.spent,"title":s.title,
            "generation":state.generation,"status":status,"query_status":query_status,"raw_rows":rows,
            "registration_no_effect":!command.stop && registration_never_started(status),
            "gamecore_base":capture.module_base,"gamecore_sha256":PINNED_GAMECORE_SHA256,
            "attempt_state":state.attempt_state,"reject_reason":state.reject_reason,"callbacks":state.callbacks,
            "observations":state.observations,"dropped":state.dropped,"execution_blockers":state.execution_blockers,
            "record_present":state.record_present,"record_generation":state.record_generation,
            "spent_before":state.spent_before,"spent_after":state.spent_after,"dispatched":state.dispatched,
            "verified":state.verified,"runtime_retained":true,"sampling_stop_only":command.stop,
            "producer_drained":false,"callback_exit_proven":false,"operation_proven":false});
        if command.stop
            && civ6_native_bridge_rs::ai_diagnostic::validate_receipt(&receipt, command).is_ok()
        {
            self.cleanup = Cleanup::Stopped;
        }
        Ok(receipt)
    }
    fn invoke_registration(
        &mut self,
        game: &Handle,
        action: usize,
        remote: usize,
        command: Command,
    ) -> Result<u32, String> {
        if command.stop {
            self.cleanup = Cleanup::StopUnconfirmed;
        } else {
            self.cleanup = Cleanup::Pending(Command {
                stop: true,
                ..command
            });
        }
        let status = remote_call(game, action, remote, "AI diagnostic registration/stop")?;
        if !command.stop && registration_never_started(status) {
            self.cleanup = Cleanup::Absent;
        }
        Ok(status)
    }
    pub(super) fn finish(
        &mut self,
        game: &Handle,
        image: BridgeImage,
        dll: &Path,
        expected_sha: &str,
    ) -> Result<(), String> {
        if let Cleanup::Pending(command) = self.cleanup {
            self.cleanup = Cleanup::StopUnconfirmed;
            // Controller is exiting; the diagnostic Session may already have
            // reserved an explicit stop whose preflight failed. Only the
            // native-call boundary consumes the pending call. A failed cleanup
            // remains unconfirmed; absence of a retry is NOT proof of success.
            if sha256(dll)? != expected_sha {
                return Err("AI diagnostic DLL identity changed before stop".into());
            }
            let receipt = self.call(command, game, image, dll)?;
            if let Some(writer) = self.writer.as_mut() {
                write_json(writer, receipt.clone())?;
            }
            civ6_native_bridge_rs::ai_diagnostic::validate_receipt(&receipt, command)?;
        }
        match self.cleanup {
            Cleanup::Absent | Cleanup::Stopped => Ok(()),
            _ => Err("AI observation stop unconfirmed; no resend; runtime retained".into()),
        }
    }
}
fn call_struct<T>(game: &Handle, entry: usize, value: &mut T) -> Result<u32, String> {
    if entry == 0 {
        return Err("AI diagnostic entry missing".into());
    }
    let remote = prepare_struct(game, value)?;
    // Never free buffers a timed-out native thread might still access.
    let status = remote_call(game, entry, remote, "AI diagnostic")?;
    let bytes =
        unsafe { std::slice::from_raw_parts_mut((value as *mut T).cast::<u8>(), size_of::<T>()) };
    read_remote(game, remote, bytes)?;
    Ok(status)
}
fn prepare_struct<T>(game: &Handle, value: &mut T) -> Result<usize, String> {
    let bytes =
        unsafe { std::slice::from_raw_parts_mut((value as *mut T).cast::<u8>(), size_of::<T>()) };
    let remote = remote_alloc(game, bytes.len())?;
    write_remote(game, remote, bytes)?;
    Ok(remote)
}
fn registration_never_started(status: u32) -> bool {
    // These returns in BridgeRuntimeAiObserve precede register_owner. Panic,
    // HookFailure, Critical, and no response do NOT prove absence of effects.
    status == BridgeStatus::InvalidInput as u32 || status == BridgeStatus::ProcessIdentity as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[repr(C)]
    struct Packet {
        calls: u32,
        marker: u32,
    }
    unsafe extern "system" fn echo(input: *mut c_void) -> u32 {
        let p = unsafe { &mut *input.cast::<Packet>() };
        p.calls += 1;
        p.marker ^= 0x53;
        23
    }
    unsafe extern "system" {
        fn BridgeRuntimeAiObserve(input: *mut c_void) -> u32;
    }
    #[test]
    fn ai_diagnostic_actual_uninstalled_export_refusal_does_not_schedule_native_stop() {
        let pid = unsafe { GetCurrentProcessId() };
        let process = open_process(
            pid,
            PROCESS_CREATE_THREAD
                | PROCESS_VM_OPERATION
                | PROCESS_VM_READ
                | PROCESS_VM_WRITE
                | PROCESS_QUERY_LIMITED_INFORMATION,
        )
        .unwrap();
        let request = json!({"action":"start_native_ai_observation","owner":2,"generation":1,"index":7,"cost":2,"paid":1,"spent":1,"title":8});
        let command = Command::parse(&request, pid, 1).unwrap();
        let mut config: AiObservationConfig = unsafe { zeroed() };
        config.size = size_of::<AiObservationConfig>() as u32;
        config.version = AI_OBSERVATION_VERSION;
        config.game_pid = pid;
        config.created_filetime = 1;
        config.owner = 2;
        config.generation = 1;
        config.governor_index = 7;
        config.expected_cost = 2;
        config.expected_paid = 1;
        config.expected_spent = 1;
        config.expected_title = 8;
        let remote = prepare_struct(&process, &mut config).unwrap();
        let mut control = Control::new();
        control.session.reserve(command).unwrap();
        let status = control
            .invoke_registration(
                &process,
                BridgeRuntimeAiObserve as *const () as usize,
                remote,
                command,
            )
            .unwrap();
        assert_eq!(status, BridgeStatus::InvalidInput as u32);
        assert_eq!(control.cleanup, Cleanup::Absent);
        assert!(control.session.reserve(command).is_err());
        control
            .finish(
                &process,
                BridgeImage {
                    base: 0,
                    size: 0,
                    gateways: [0; 3],
                },
                Path::new("absent-no-load.dll"),
                "unused",
            )
            .unwrap();
        // No response cannot clear a potential effect or permit retry.
        assert!(
            control
                .invoke_registration(&process, 0, remote, command)
                .is_err()
        );
        assert!(matches!(control.cleanup, Cleanup::Pending(_)));
        for status in [
            BridgeStatus::Panic,
            BridgeStatus::HookFailure,
            BridgeStatus::Critical,
            BridgeStatus::Ok,
        ] {
            assert!(!registration_never_started(status as u32));
        }
    }
    #[test]
    fn ai_diagnostic_os_transport_invokes_once_and_reads_actual_native_result() {
        // Current test process only; no game/module lookup or registration.
        let process = open_process(
            unsafe { GetCurrentProcessId() },
            PROCESS_CREATE_THREAD
                | PROCESS_VM_OPERATION
                | PROCESS_VM_READ
                | PROCESS_VM_WRITE
                | PROCESS_QUERY_LIMITED_INFORMATION,
        )
        .unwrap();
        let mut packet = Packet {
            calls: 0,
            marker: 0x1234,
        };
        assert_eq!(
            call_struct(&process, echo as *const () as usize, &mut packet).unwrap(),
            23
        );
        assert_eq!((packet.calls, packet.marker), (1, 0x1234 ^ 0x53));
        assert!(call_struct(&process, 0, &mut packet).is_err());
        assert_eq!(packet.calls, 1);
    }
    #[test]
    fn ai_diagnostic_stop_preflight_keeps_cleanup_but_crossed_call_never_retries() {
        let pid = unsafe { GetCurrentProcessId() };
        let process = open_process(
            pid,
            PROCESS_CREATE_THREAD
                | PROCESS_VM_OPERATION
                | PROCESS_VM_READ
                | PROCESS_VM_WRITE
                | PROCESS_QUERY_LIMITED_INFORMATION,
        )
        .unwrap();
        let start=Command::parse(&json!({"action":"start_native_ai_observation","owner":2,"generation":1,"index":7,"cost":2,"paid":1,"spent":1,"title":8}),pid,1).unwrap();
        let stop = Command {
            stop: true,
            ..start
        };
        let mut control = Control::new();
        control.session.reserve(start).unwrap();
        control.cleanup = Cleanup::Pending(stop); // controller state after an installed/unknown start
        control.session.reserve(stop).unwrap();
        let image = BridgeImage {
            base: 0,
            size: 0,
            gateways: [0; 3],
        };
        assert!(
            control
                .call(
                    stop,
                    &process,
                    image,
                    Path::new("absent-ai-stop-preflight.dll")
                )
                .is_err()
        );
        assert_eq!(control.cleanup, Cleanup::Pending(stop));
        // The real OS transport boundary clears pending exactly once, including
        // a non-success return. No constant-success native adapter is supplied.
        let mut packet = Packet {
            calls: 0,
            marker: 7,
        };
        let remote = prepare_struct(&process, &mut packet).unwrap();
        assert_eq!(
            control
                .invoke_registration(&process, echo as *const () as usize, remote, stop)
                .unwrap(),
            23
        );
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                (&mut packet as *mut Packet).cast::<u8>(),
                size_of::<Packet>(),
            )
        };
        read_remote(&process, remote, bytes).unwrap();
        assert_eq!(packet.calls, 1);
        assert_eq!(control.cleanup, Cleanup::StopUnconfirmed);
        assert!(
            control
                .finish(
                    &process,
                    image,
                    Path::new("absent-ai-stop-preflight.dll"),
                    "unused"
                )
                .unwrap_err()
                .contains("unconfirmed")
        );
        assert!(
            control
                .finish(
                    &process,
                    image,
                    Path::new("absent-ai-stop-preflight.dll"),
                    "unused"
                )
                .is_err()
        );
        assert!(control.session.reserve(stop).is_err());
        control.cleanup = Cleanup::Pending(stop);
        // A failed automatic preflight must report failure, not successful stop
        // or a spurious second Session reservation before attempting cleanup.
        let error = control
            .finish(
                &process,
                image,
                Path::new("absent-ai-stop-preflight.dll"),
                "unused",
            )
            .unwrap_err();
        assert!(!error.contains("already attempted"));
    }
}
