//! Guarded runtime entry of the Oath observer: configuration, status codes,
//! a one-shot lifecycle, a gate port for the identity preflight, and the four
//! exports (Install, StartObservation, StopObservation, ReadReceipt).
//!
//! Install writes nothing to game memory (and nothing to the caller's config)
//! until the gate has verified the process, the executable, the GameCore
//! module and every wrapper byte. The deep identity checks (disk SHA, module
//! path, MEM_IMAGE ownership, wrapper bytes, function tables) belong to the
//! preflight and plug in through [`Gate`]; the default gate performs the own
//! process checks and then reports `NotWired`, so a DLL built from this
//! crate alone can never install. The mode is diagnostic only and no field
//! here can accept a quantity.
use crate::native::*;
use crate::oath_record::{OathCall, OATH_ENTRY_HEAD};
use crate::oath_trace::{install_oath_at, uninstall_oath, OathTrace};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetProcessTimes};

pub const OATH_RUNTIME_VERSION: u32 = 0x0134_0001;
/// The only accepted mode: observe and report, never adjust anything.
pub const OATH_MODE_DIAGNOSTIC: u32 = 1;
pub const OATH_MAX_ACTIVE_SECONDS: u32 = 300;

/// Public input and receipt in one 48 byte block. The two out fields must be
/// zero on input; they are written once, after a successful install.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OathRuntimeConfig {
    pub version: u32,
    pub size: u32,
    pub game_pid: u32,
    pub max_seconds: u32,
    pub created_filetime: u64,
    pub mode: u32,
    pub flags: u32,
    pub out_trace_address: u64,
    pub trace_size: u32,
    pub reserved: u32,
}
const _: () = assert!(std::mem::size_of::<OathRuntimeConfig>() == 48);

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OathStatus {
    Ok = 0,
    InvalidInput = 1,
    ProcessIdentity = 2,
    ImageIdentity = 3,
    EntryBytes = 4,
    HookFailure = 5,
    Critical = 6,
    WrongState = 7,
    NotWired = 8,
    Panic = 99,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Fresh = 0,
    Installed = 1,
    Started = 2,
    Stopped = 3,
    Failed = 4,
}

/// One-shot lifecycle: install once, start once, stop for good. A start after
/// a stop, a second install and any use after a failed install are refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lifecycle {
    phase: Phase,
}

impl Lifecycle {
    pub const fn new() -> Self {
        Self {
            phase: Phase::Fresh,
        }
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn can_install(&self) -> bool {
        self.phase == Phase::Fresh
    }
    /// Install reached game memory successfully.
    pub fn installed(&mut self) -> Result<(), OathStatus> {
        self.step(Phase::Fresh, Phase::Installed)
    }
    /// Install failed after it may have touched game memory: terminal.
    pub fn failed(&mut self) -> Result<(), OathStatus> {
        self.step(Phase::Fresh, Phase::Failed)
    }
    pub fn start(&mut self) -> Result<(), OathStatus> {
        self.step(Phase::Installed, Phase::Started)
    }
    /// Stops from Installed or Started; stopping again is not an error.
    pub fn stop(&mut self) -> Result<(), OathStatus> {
        match self.phase {
            Phase::Installed | Phase::Started | Phase::Stopped => {
                self.phase = Phase::Stopped;
                Ok(())
            }
            _ => Err(OathStatus::WrongState),
        }
    }
    fn step(&mut self, from: Phase, to: Phase) -> Result<(), OathStatus> {
        if self.phase == from {
            self.phase = to;
            Ok(())
        } else {
            Err(OathStatus::WrongState)
        }
    }
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}

/// Pure input validation, no process access.
pub fn validate(config: &OathRuntimeConfig) -> Result<(), OathStatus> {
    if config.version != OATH_RUNTIME_VERSION
        || config.size as usize != std::mem::size_of::<OathRuntimeConfig>()
        || config.game_pid == 0
        || config.created_filetime == 0
        || !(1..=OATH_MAX_ACTIVE_SECONDS).contains(&config.max_seconds)
        || config.mode != OATH_MODE_DIAGNOSTIC
        || config.flags != 0
        || config.out_trace_address != 0
        || config.trace_size != 0
        || config.reserved != 0
    {
        return Err(OathStatus::InvalidInput);
    }
    Ok(())
}

/// What the identity preflight proved, as addresses inside the verified image.
#[derive(Clone, Copy, Debug)]
pub struct Verified {
    pub image_base: usize,
    /// An address inside the verified GameCore module, to be pinned.
    pub core_module: usize,
    pub apply_entry: usize,
    pub remove_entry: usize,
}

/// Port for the deep identity checks. It must not write anything.
pub trait Gate {
    fn verify(&self, config: &OathRuntimeConfig) -> Result<Verified, OathStatus>;
}

pub fn current_created_filetime() -> Option<u64> {
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    let ok = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    } != 0;
    ok.then(|| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

/// The checks that need no file: this is the process the caller named.
pub fn own_process(config: &OathRuntimeConfig) -> Result<(), OathStatus> {
    if unsafe { GetCurrentProcessId() } != config.game_pid
        || current_created_filetime() != Some(config.created_filetime)
    {
        return Err(OathStatus::ProcessIdentity);
    }
    Ok(())
}

/// Own-process checks, then `NotWired` until the preflight is connected.
pub struct DefaultGate;
impl Gate for DefaultGate {
    fn verify(&self, config: &OathRuntimeConfig) -> Result<Verified, OathStatus> {
        own_process(config)?;
        Err(OathStatus::NotWired)
    }
}

struct Runtime {
    lifecycle: Lifecycle,
    trace: Option<&'static OathTrace>,
    max_seconds: u32,
    started: Option<Instant>,
}

static RUNTIME: Mutex<Runtime> = Mutex::new(Runtime {
    lifecycle: Lifecycle::new(),
    trace: None,
    max_seconds: 0,
    started: None,
});

impl Runtime {
    /// Past its bounded time a running observation stops itself lazily.
    fn apply_deadline(&mut self) {
        let expired = self
            .started
            .is_some_and(|at| at.elapsed() > Duration::from_secs(u64::from(self.max_seconds)));
        if expired && self.lifecycle.phase() == Phase::Started {
            let _ = self.lifecycle.stop();
            if let Some(trace) = self.trace {
                trace.stop();
            }
        }
    }
}

fn pin_address(address: usize) -> bool {
    let mut module = null_mut();
    let ok = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            address as *const u16,
            &mut module,
        )
    };
    ok != 0 && !module.is_null()
}

fn hook_status(error: &str) -> OathStatus {
    if error.starts_with("critical") {
        OathStatus::Critical
    } else {
        OathStatus::HookFailure
    }
}

/// Install with an explicit gate (the export uses [`DefaultGate`]).
///
/// Order: validate the block, own state, gate (reads only), pin the game
/// module and this DLL, allocate an inactive trace, hook Apply then Remove,
/// and only then write the receipt fields into the caller's block.
pub fn install_with(gate: &dyn Gate, input: *mut c_void) -> OathStatus {
    if !committed_range(
        input as usize,
        std::mem::size_of::<OathRuntimeConfig>(),
        std::mem::align_of::<OathRuntimeConfig>(),
        true,
    ) {
        return OathStatus::InvalidInput;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| install_locked(gate, input)))
        .unwrap_or(OathStatus::Panic)
}

fn install_locked(gate: &dyn Gate, input: *mut c_void) -> OathStatus {
    let Ok(mut runtime) = RUNTIME.lock() else {
        return OathStatus::Panic;
    };
    let mut config = unsafe { ptr::read(input.cast::<OathRuntimeConfig>()) };
    if let Err(status) = validate(&config) {
        return status;
    }
    if !runtime.lifecycle.can_install() {
        return OathStatus::WrongState;
    }
    let verified = match gate.verify(&config) {
        Ok(verified) => verified,
        Err(status) => return status,
    };
    // Pin before any executable page is published: a gateway or a fetched
    // jump may outlive an install that fails half way.
    if !pin_address(verified.core_module) || !pin_address(install_locked as *const () as usize) {
        return OathStatus::HookFailure;
    }
    let trace: &'static OathTrace = Box::leak(OathTrace::boxed_inactive());
    trace.set_image_base_once(verified.image_base as u64);
    let hook = |entry: usize, call: OathCall| unsafe {
        install_oath_at(entry as *mut u8, call, trace, OATH_ENTRY_HEAD)
    };
    let result = hook(verified.apply_entry, OathCall::Apply)
        .and_then(|()| hook(verified.remove_entry, OathCall::Remove));
    if let Err(error) = result {
        // A half installed pair is taken back as far as the ledger allows;
        // the call type stays bound and the lifecycle is terminal.
        let _ = unsafe { uninstall_oath() };
        let _ = runtime.lifecycle.failed();
        return hook_status(error);
    }
    config.out_trace_address = trace as *const OathTrace as u64;
    config.trace_size = std::mem::size_of::<OathTrace>() as u32;
    unsafe { ptr::write(input.cast::<OathRuntimeConfig>(), config) };
    runtime.trace = Some(trace);
    runtime.max_seconds = config.max_seconds;
    let _ = runtime.lifecycle.installed();
    OathStatus::Ok
}

/// Starts recording, once. Call it before the official choice is sent.
pub fn start_observation() -> OathStatus {
    transition(|runtime| {
        runtime.apply_deadline();
        runtime.lifecycle.start()?;
        let trace = runtime.trace.ok_or(OathStatus::WrongState)?;
        if !trace.start_once() {
            return Err(OathStatus::Critical);
        }
        runtime.started = Some(Instant::now());
        Ok(())
    })
}

/// Stops recording for good. The hooks, relays and trace stay resident.
pub fn stop_observation() -> OathStatus {
    transition(|runtime| {
        runtime.lifecycle.stop()?;
        if let Some(trace) = runtime.trace {
            trace.stop();
        }
        Ok(())
    })
}

fn transition(step: impl FnOnce(&mut Runtime) -> Result<(), OathStatus>) -> OathStatus {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Ok(mut runtime) = RUNTIME.lock() else {
            return OathStatus::Panic;
        };
        step(&mut runtime).map_or_else(|status| status, |()| OathStatus::Ok)
    }))
    .unwrap_or(OathStatus::Panic)
}

/// Counters and state in one 64 byte block. `quantity_accepted` is
/// always zero: nothing here is a quantity.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OathReceipt {
    pub version: u32,
    pub size: u32,
    pub phase: u32,
    pub flags: u32,
    pub trace_address: u64,
    pub trace_size: u32,
    pub max_seconds: u32,
    pub rows: u32,
    pub completed: u32,
    pub dropped: u32,
    pub filtered: u32,
    pub in_flight: u32,
    pub expired: u32,
    pub quantity_accepted: u32,
    pub reserved: u32,
}
const _: () = assert!(std::mem::size_of::<OathReceipt>() == 64);

pub fn read_receipt() -> Result<OathReceipt, OathStatus> {
    let Ok(mut runtime) = RUNTIME.lock() else {
        return Err(OathStatus::Panic);
    };
    runtime.apply_deadline();
    let mut receipt = OathReceipt {
        version: OATH_RUNTIME_VERSION,
        size: std::mem::size_of::<OathReceipt>() as u32,
        phase: runtime.lifecycle.phase() as u32,
        max_seconds: runtime.max_seconds,
        ..OathReceipt::default()
    };
    if let Some(trace) = runtime.trace {
        let (rows, completed, dropped) = trace.counters();
        receipt.trace_address = trace as *const OathTrace as u64;
        receipt.trace_size = std::mem::size_of::<OathTrace>() as u32;
        receipt.rows = rows;
        receipt.completed = completed;
        receipt.dropped = dropped;
        receipt.filtered = trace.filtered();
        receipt.in_flight = trace.in_flight();
        receipt.expired =
            u32::from(runtime.started.is_some_and(|at| {
                at.elapsed() > Duration::from_secs(u64::from(runtime.max_seconds))
            }));
    }
    Ok(receipt)
}

#[unsafe(no_mangle)]
pub extern "system" fn OathRuntimeInstall(input: *mut c_void) -> u32 {
    install_with(&DefaultGate, input) as u32
}

#[unsafe(no_mangle)]
pub extern "system" fn OathRuntimeStartObservation(_: *mut c_void) -> u32 {
    start_observation() as u32
}

#[unsafe(no_mangle)]
pub extern "system" fn OathRuntimeStopObservation(_: *mut c_void) -> u32 {
    stop_observation() as u32
}

#[unsafe(no_mangle)]
pub extern "system" fn OathRuntimeReadReceipt(output: *mut c_void) -> u32 {
    if !committed_range(
        output as usize,
        std::mem::size_of::<OathReceipt>(),
        std::mem::align_of::<OathReceipt>(),
        true,
    ) {
        return OathStatus::InvalidInput as u32;
    }
    match std::panic::catch_unwind(read_receipt) {
        Ok(Ok(receipt)) => {
            unsafe { ptr::write(output.cast::<OathReceipt>(), receipt) };
            OathStatus::Ok as u32
        }
        Ok(Err(status)) => status as u32,
        Err(_) => OathStatus::Panic as u32,
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
