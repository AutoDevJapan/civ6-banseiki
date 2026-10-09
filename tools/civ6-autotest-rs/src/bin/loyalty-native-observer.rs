//! Bounded, external x64 observer. No game commands, injection, or memory writes.
//!
//! Run only against the PID/creation FILETIME supplied by the live_chain owner.
//! `ready` (stdout and OUTPUT.ready) means that the three setter/candidate probes
//! are installed. Getter is enabled only on the retained first setter thread.
//! Output contains raw fields only. Even a completed observation is not product
//! acceptance. Forced termination of this debugger cannot run Rust cleanup;
//! use its deadline or Ctrl-C for an orderly restore/continue/detach instead.
//! Pointer types, numeric scale, getter mapping, and notification semantics
//! remain unverified; raw read errors are samples, not target exceptions.

use clap::Parser;
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
use serde::Serialize;
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
use std::path::Path;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Parser)]
#[command(about = "Observe one pinned Civ6 process using bounded hardware execution probes")]
struct Args {
    #[arg(long)]
    pid: u32,
    /// Decimal Windows creation FILETIME (not Unix time).
    #[arg(long)]
    created: u64,
    /// Absolute, previously nonexistent JSONL file; OUTPUT.ready must also be new.
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = 90)]
    seconds: u64,
}

#[cfg(any(all(windows, target_arch = "x86_64"), test))]
impl Args {
    fn validate(&self) -> Result<()> {
        if self.pid == 0 || self.created == 0 {
            return Err("--pid and --created must be nonzero".into());
        }
        if !(1..=90).contains(&self.seconds) {
            return Err("--seconds must be in 1..=90".into());
        }
        if !self.output.is_absolute()
            || self.output.extension().and_then(|s| s.to_str()) != Some("jsonl")
        {
            return Err("--output must be an absolute path ending in .jsonl".into());
        }
        Ok(())
    }

    fn ready_path(&self) -> PathBuf {
        let mut path = self.output.as_os_str().to_os_string();
        path.push(".ready");
        PathBuf::from(path)
    }
}

// These helpers also compile on non-Windows, so safety policies can be tested
// without attaching, launching, or inspecting any real process.
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
mod policy {
    use super::attach_image::VerifiedAttachExports;
    use super::*;
    use std::collections::BTreeMap;

    pub const MODULE_NAME: &str = "GameCore_XP2_FinalRelease.dll";
    pub const MODULE_SHA256: &str =
        "324C51E9EA3531758842E16C69E6CDDBEFBB226C5B675C3D6E60111646C2E98C";
    pub const RVAS: [u64; 4] = [0x713E3D, 0x713E42, 0x70AF4A, 0x115357];
    pub const RF: u32 = 1 << 16;
    pub const GETTER_LIMIT: u32 = 32;
    pub const CANDIDATE_LIMIT: u32 = 8;

    #[derive(Default, Serialize)]
    pub struct AttachBreak {
        image: Option<VerifiedAttachExports>,
        pub breakin_threads: BTreeMap<u32, u64>,
        consumed: bool,
        registration_failed: bool,
    }

    impl AttachBreak {
        pub fn bind(&mut self, image: VerifiedAttachExports) {
            self.image = Some(image);
        }

        pub fn register(
            &mut self,
            tid: u32,
            event_start: Option<u64>,
            os_start: Result<u64>,
            existed_before_attach: bool,
        ) -> Result<bool> {
            let start = os_start?;
            let Some(image) = &self.image else {
                return Ok(false);
            };
            if self.consumed
                || self.registration_failed
                || existed_before_attach
                || start != image.breakin_address()
                || event_start.is_some_and(|event| event != start)
            {
                return Ok(false);
            }
            if tid == 0
                || (!self.breakin_threads.is_empty() && !self.breakin_threads.contains_key(&tid))
            {
                self.registration_failed = true;
                self.breakin_threads.clear();
                return Err("ambiguous debugger break-in thread registration".into());
            }
            self.breakin_threads.insert(tid, start);
            Ok(true)
        }

        pub fn retire(&mut self, tid: u32) {
            self.breakin_threads.remove(&tid);
        }

        pub fn matches(&self, exception: RawException) -> bool {
            self.image.as_ref().is_some_and(|image| {
                !self.consumed
                    && !self.registration_failed
                    && exception.code == 0x80000003
                    && exception.first_chance != 0
                    && exception.exception_address == image.breakpoint_address()
                    && self.breakin_threads.get(&exception.tid) == Some(&image.breakin_address())
            })
        }

        pub fn consume(&mut self, exception: RawException) -> bool {
            if !self.consumed && self.matches(exception) {
                self.consumed = true;
                true
            } else {
                false
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
    pub struct Identity {
        pub pid: u32,
        pub created: u64,
        pub image_path: String,
    }

    pub fn match_identity(expected: &Identity, actual: &Identity, alive: bool) -> Result<()> {
        if !alive {
            return Err("pinned process has exited".into());
        }
        if expected != actual {
            return Err("PID, creation FILETIME, or pinned image path changed".into());
        }
        Ok(())
    }

    pub fn expected_module(image: &Path) -> Result<PathBuf> {
        let file = image.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !["CivilizationVI.exe", "CivilizationVI_DX12.exe"]
            .iter()
            .any(|name| file.eq_ignore_ascii_case(name))
        {
            return Err("target image is not a supported Civ6 executable".into());
        }
        let mut root = image.parent().ok_or("image has no parent")?;
        for suffix in ["Win64Steam", "Binaries", "Base"] {
            if !root
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|part| part.eq_ignore_ascii_case(suffix))
            {
                return Err(
                    "image is outside the supported Base/Binaries/Win64Steam layout".into(),
                );
            }
            root = root.parent().ok_or("incomplete Civ6 image path")?;
        }
        Ok(root.join("DLC/Expansion2/Binaries/Win64").join(MODULE_NAME))
    }

    pub fn breakpoint_addresses(base: u64, size: u32) -> Result<[u64; 4]> {
        if base == 0 || RVAS.iter().any(|&rva| rva >= u64::from(size)) {
            return Err("probe RVAs are outside the pinned module".into());
        }
        let mut addresses = [0; 4];
        for (address, rva) in addresses.iter_mut().zip(RVAS) {
            *address = base.checked_add(rva).ok_or("module address overflow")?;
        }
        Ok(addresses)
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
    pub struct DebugRegisters {
        pub address: [u64; 4],
        pub dr6: u64,
        pub dr7: u64,
    }

    impl DebugRegisters {
        pub fn unused(self) -> bool {
            // DR6's architectural reset/reserved ones and DR7's fixed bit 10
            // may be returned by Windows even when no debugger uses the thread.
            // All addresses, control bits, and meaningful status must be zero.
            self.address == [0; 4] && matches!(self.dr6, 0 | 0xFFFF0FF0) && self.dr7 & !0x400 == 0
        }

        pub fn installed(self, addresses: [u64; 4], enabled: u64) -> Self {
            Self {
                address: std::array::from_fn(|slot| {
                    if enabled & (1 << (slot * 2)) != 0 {
                        addresses[slot]
                    } else {
                        0
                    }
                }),
                dr6: self.dr6,
                dr7: self.dr7 | enabled, // local execution probes, length 1
            }
        }

        pub fn own_hit(
            self,
            addresses: [u64; 4],
            enabled: u64,
            rip: u64,
            flags: u32,
        ) -> Option<ProbeSource> {
            // B0..B3 are sticky CPU status, not a one-hot event ID. Select the
            // current execution slot from RIP, then check that slot's B bit.
            let slot = addresses.iter().position(|&address| address == rip)?;
            if enabled & (1 << (slot * 2)) == 0
                || self.address != Self::default().installed(addresses, enabled).address
                || self.dr7 & !0x400 != enabled
                || self.dr6 & (1 << slot) == 0
            {
                return None;
            }
            let owned_b_bits = self.dr6 & enabled_b_bits(enabled);
            Some(ProbeSource {
                slot,
                owned_b_bits,
                app_dr6: self.dr6 & 0xE00F & !owned_b_bits,
                app_tf: flags & 0x100 != 0,
            })
        }
    }

    pub fn enabled_b_bits(enabled: u64) -> u64 {
        (0..4).fold(0, |bits, slot| {
            bits | (((enabled >> (slot * 2)) & 1) << slot)
        })
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
    pub struct ProbeSource {
        pub slot: usize,
        pub owned_b_bits: u64,
        pub app_dr6: u64,
        // Future single-step setting, not evidence of a current BS trap.
        pub app_tf: bool,
    }

    impl ProbeSource {
        fn has_current_foreign_source(self) -> bool {
            // Intel SDM Vol. 3B 18.3.1.1/18.3.1.4: an execution
            // breakpoint is a fault BEFORE the instruction; BS identifies the
            // AFTER-instruction TF trap. TF alone is the next-step setting.
            self.app_dr6 != 0
        }
    }

    pub const FIRST_CHANCE_RECORD_LIMIT: u32 = 16;

    #[derive(Clone, Copy, Debug, Serialize)]
    pub struct ProbeSnapshot {
        pub armed: bool,
        pub registers: DebugRegisters,
        pub rip: u64,
        pub flags: u32,
    }

    #[derive(Clone, Copy, Debug, Serialize)]
    pub struct RawException {
        pub tid: u32,
        pub code: u32,
        pub first_chance: u32,
        pub exception_address: u64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
    pub enum ExceptionDecision {
        AttachBreak,
        Probe(ProbeSource),
        ProbeAndStopForAppTF(ProbeSource),
        ForwardComposite(ProbeSource),
        RestoreAndForward,
        ForwardFirstChance { record: bool },
        ForwardSecondChance,
    }

    impl ExceptionDecision {
        pub fn handled(self) -> bool {
            matches!(
                self,
                Self::AttachBreak | Self::Probe(_) | Self::ProbeAndStopForAppTF(_)
            )
        }

        pub fn stop_observing(self) -> bool {
            matches!(
                self,
                Self::ProbeAndStopForAppTF(_)
                    | Self::ForwardComposite(_)
                    | Self::RestoreAndForward
                    | Self::ForwardSecondChance
            )
        }

        pub fn resume_debug_flags(self, dr6: u64, flags: u32) -> Option<(u64, u32)> {
            match self {
                Self::Probe(source)
                | Self::ProbeAndStopForAppTF(source)
                | Self::ForwardComposite(source) => Some((dr6 & !source.owned_b_bits, flags | RF)),
                _ => None,
            }
        }
    }

    #[derive(Default, Serialize)]
    pub struct ExceptionPolicy {
        pub attach: AttachBreak,
        pub unknown_first_chance_count: u64,
        pub unknown_first_chance_records: u32,
        pub second_chance_count: u64,
    }

    impl ExceptionPolicy {
        pub fn decide(
            &mut self,
            exception: RawException,
            snapshot: Option<ProbeSnapshot>,
            addresses: [u64; 4],
            enabled: u64,
        ) -> ExceptionDecision {
            // Chance takes precedence over EVERY source/identity check. A
            // target's unhandled exception is never an observer-owned event.
            if exception.first_chance == 0 {
                self.second_chance_count = self.second_chance_count.saturating_add(1);
                return ExceptionDecision::ForwardSecondChance;
            }
            if self.attach.consume(exception) {
                return ExceptionDecision::AttachBreak;
            }
            if exception.code == 0x80000004 && enabled != 0 {
                let uncertain = || ExceptionDecision::RestoreAndForward;
                let Some(snapshot) = snapshot.filter(|s| s.armed) else {
                    self.record_first_chance();
                    return uncertain();
                };
                let expected = DebugRegisters::default().installed(addresses, enabled);
                if snapshot.registers.address != expected.address
                    || snapshot.registers.dr7 & !0x400 != enabled
                {
                    self.record_first_chance();
                    return uncertain();
                }
                if snapshot.rip == exception.exception_address {
                    if let Some(source) =
                        snapshot
                            .registers
                            .own_hit(addresses, enabled, snapshot.rip, snapshot.flags)
                    {
                        if source.has_current_foreign_source() {
                            self.record_first_chance();
                            return ExceptionDecision::ForwardComposite(source);
                        }
                        if source.app_tf {
                            return ExceptionDecision::ProbeAndStopForAppTF(source);
                        }
                        return ExceptionDecision::Probe(source);
                    }
                }
                // A disabled-slot/foreign trap with no owned B bit is known
                // foreign. Missing evidence at an owned RIP, sticky owned bits,
                // or an ExceptionAddress mismatch cannot authorize handling.
                if snapshot.registers.dr6 & enabled_b_bits(enabled) != 0
                    || addresses.iter().enumerate().any(|(slot, &address)| {
                        enabled & (1 << (slot * 2)) != 0 && snapshot.rip == address
                    })
                {
                    self.record_first_chance();
                    return uncertain();
                }
            }
            ExceptionDecision::ForwardFirstChance {
                record: self.record_first_chance(),
            }
        }

        fn record_first_chance(&mut self) -> bool {
            self.unknown_first_chance_count = self.unknown_first_chance_count.saturating_add(1);
            let record = self.unknown_first_chance_records < FIRST_CHANCE_RECORD_LIMIT;
            if record {
                self.unknown_first_chance_records += 1;
            }
            record
        }
    }

    pub fn validate_resume_readback(
        requested: ProbeSnapshot,
        actual: Result<ProbeSnapshot>,
    ) -> Result<()> {
        let actual = actual.map_err(|error| format!("probe resume readback failed: {error}"))?;
        let mut differences = Vec::new();
        if actual.registers.dr6 & 0xE00F != requested.registers.dr6 & 0xE00F {
            differences.push("DR6 B0..B3/BD/BS/BT");
        }
        if actual.registers.dr7 & !0x400 != requested.registers.dr7 & !0x400
            || actual.registers.address != requested.registers.address
        {
            differences.push("DR0..DR3/DR7");
        }
        if actual.rip != requested.rip {
            differences.push("RIP");
        }
        if actual.flags & RF != requested.flags & RF {
            differences.push("RF");
        }
        // EFLAGS bit 1 is an architectural fixed one, not application state.
        // Windows returns it as zero after SetThreadContext in the owned-child
        // test. Retain raw values in the trace; never synthesize an RF value.
        if actual.flags & !(RF | 2) != requested.flags & !(RF | 2) {
            differences.push("application EFlags");
        }
        if differences.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "probe resume readback mismatch: {}; requested EFlags={:#x}, actual EFlags={:#x}; requested DR6={:#x}, actual DR6={:#x}",
                differences.join(", "), requested.flags, actual.flags, requested.registers.dr6, actual.registers.dr6
            ))
        }
    }

    pub fn trace_priority(record: &serde_json::Value) -> u8 {
        if record["ok"] == false
            || record["readback_ok"] == false
            || record["set_context_ok"] == false
            || record["event"] == "error"
            || record["event"] == "exit"
        {
            return 3;
        }
        match record["event"].as_str() {
            Some("probe_install") => 0,
            Some(
                "probe_resume" | "debug_continue" | "target_exit_wait" | "probe_restore"
                | "thread_resume",
            ) => 2,
            _ => 1,
        }
    }

    pub fn retain_trace(
        records: &mut Vec<serde_json::Value>,
        omitted: &mut BTreeMap<String, u64>,
        record: serde_json::Value,
    ) -> bool {
        if records.len() < 128 {
            records.push(record);
            return false;
        }
        let priority = trace_priority(&record);
        let victim = records
            .iter()
            .enumerate()
            .filter(|(_, old)| trace_priority(old) < priority)
            .min_by_key(|(_, old)| trace_priority(old))
            .map(|(i, _)| i);
        let (lost, reason) = if let Some(index) = victim {
            let lost = records.remove(index);
            records.push(record);
            (lost, "replaced_by_priority")
        } else {
            (record, "capacity")
        };
        let key = format!("{}/{}", lost["event"].as_str().unwrap_or("unknown"), reason);
        let count = omitted.entry(key).or_default();
        *count = count.saturating_add(1);
        true
    }

    pub fn require_ready(outcome: Result<&'static str>, ready: bool) -> Result<&'static str> {
        match outcome {
            Ok(reason) if !ready => Err(format!("observation ended before ready: {reason}")),
            other => other,
        }
    }

    // A remote read error is diagnostic data, not a Windows exception from the
    // target. No caller can propagate it as an observation/session failure.
    pub fn raw_read<T: Serialize>(result: Result<T>) -> serde_json::Value {
        match result {
            Ok(value) => serde_json::to_value(value).unwrap_or_else(|error|
                serde_json::json!({"read_error":format!("raw field serialization: {error}")})),
            Err(error) => serde_json::json!({"read_error":error}),
        }
    }

    #[derive(Clone, Copy, Debug, Serialize)]
    pub struct Trigger {
        pub cache_city: u64,
        pub receiver: u64,
        pub tid: u32,
        pub rsp: u64,
        pub returned: bool,
    }

    #[derive(Debug, Default, Serialize)]
    pub struct Observation {
        pub trigger: Option<Trigger>,
        pub later_setters_ignored: u64,
        pub another_setter_for_trigger_cache: bool,
        pub getters_before_return: u64,
        pub getters_recorded: u32,
        pub candidates_recorded: u32,
    }

    impl Observation {
        pub fn setter(&mut self, cache: u64, rcx: u64, tid: u32, rsp: u64) -> Result<bool> {
            if let Some(trigger) = self.trigger {
                self.later_setters_ignored = self.later_setters_ignored.saturating_add(1);
                self.another_setter_for_trigger_cache |= trigger.cache_city == cache;
                return Ok(false);
            }
            if cache == 0 || cache.checked_add(0x1630) != Some(rcx) {
                return Err("first setter RCX does not equal RBX + 0x1630".into());
            }
            self.trigger = Some(Trigger {
                cache_city: cache,
                receiver: rcx,
                tid,
                rsp,
                returned: false,
            });
            Ok(true)
        }

        pub fn returned(&mut self, cache: u64, tid: u32, rsp: u64) -> bool {
            let Some(trigger) = self.trigger.as_mut() else {
                return false;
            };
            if trigger.returned
                || (trigger.cache_city, trigger.tid, trigger.rsp) != (cache, tid, rsp)
            {
                return false;
            }
            trigger.returned = true;
            true
        }

        pub fn getter(&mut self) -> bool {
            if !self.trigger.is_some_and(|trigger| trigger.returned) {
                self.getters_before_return = self.getters_before_return.saturating_add(1);
                return false;
            }
            // Getter and setter receivers have different types/resolution
            // paths. Only temporal correlation is known; do not infer a cache
            // offset or any city's identity from the getter's RAX.
            if self.getters_recorded >= GETTER_LIMIT {
                return false;
            }
            self.getters_recorded += 1;
            true
        }

        pub fn candidate(&mut self, receiver: u64, tid: u32) -> bool {
            if !self.trigger.is_some_and(|trigger| {
                !trigger.returned && trigger.receiver == receiver && trigger.tid == tid
            }) || self.candidates_recorded >= CANDIDATE_LIMIT
            {
                return false;
            }
            self.candidates_recorded += 1;
            true
        }
    }

    #[derive(Debug, Serialize, PartialEq, Eq)]
    pub enum RestoreOutcome {
        Restored,
        ThreadExited,
    }

    #[derive(Debug, Default, Serialize)]
    pub struct CleanupReport {
        pub threads_restored: u32,
        pub threads_exited: u32,
        pub errors: Vec<String>,
        pub continue_pending_ok: bool,
        pub debug_active_process_stop_ok: bool,
        pub debug_active_process_stop_error: Option<String>,
        pub process_exited: bool,
    }

    impl CleanupReport {
        pub fn complete(&self) -> bool {
            self.errors.is_empty()
                && self.continue_pending_ok
                && (self.debug_active_process_stop_ok || self.process_exited)
        }
    }

    pub trait CleanupBackend {
        fn freeze(&mut self) -> Vec<String>;
        fn armed_threads(&self) -> Vec<u32>;
        fn restore(&mut self, tid: u32) -> Result<RestoreOutcome>;
        fn continue_pending(&mut self) -> Result<()>;
        fn drain_events(&mut self) -> Result<()>;
        fn detach(&mut self) -> Result<bool>;
        fn detach_diagnostic(&self) -> Option<String> {
            None
        }
        fn resume(&mut self) -> Vec<String>;
        fn process_exited(&self) -> bool;
    }

    pub fn cleanup(backend: &mut impl CleanupBackend) -> CleanupReport {
        let mut report = CleanupReport {
            errors: backend.freeze(),
            ..Default::default()
        };
        // No `?` here: one failed restore must not skip other threads, the
        // outstanding event, or the detach attempt. No log I/O in this path.
        for tid in backend.armed_threads() {
            match backend.restore(tid) {
                Ok(RestoreOutcome::Restored) => report.threads_restored += 1,
                Ok(RestoreOutcome::ThreadExited) => report.threads_exited += 1,
                Err(error) => report.errors.push(format!("restore thread {tid}: {error}")),
            }
        }
        match backend.continue_pending() {
            Ok(()) => report.continue_pending_ok = true,
            Err(error) => report.errors.push(error),
        }
        if let Err(error) = backend.drain_events() {
            report.errors.push(error);
        }
        match backend.detach() {
            Ok(ok) => report.debug_active_process_stop_ok = ok,
            Err(error) => report.errors.push(error),
        }
        report.debug_active_process_stop_error = backend.detach_diagnostic();
        report.errors.extend(backend.resume());
        report.process_exited = backend.process_exited();
        report
    }
}

// Parse the trusted on-disk system DLL, then compare only specified small
// metadata fields and export prefixes in its loaded image. No remote PE scan.
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
mod attach_image {
    use super::*;

    pub const EXPORT_NAMES: [&str; 3] = [
        "DbgBreakPoint",
        "DbgUiRemoteBreakin",
        "NtQueryInformationThread",
    ];
    pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;

    pub trait ShortImageReader {
        fn read_field(&self, address: u64, length: usize) -> Result<Vec<u8>>;
    }

    #[derive(Debug, Serialize)]
    pub struct ImageField {
        pub rva: u32,
        pub bytes: Vec<u8>,
    }

    #[derive(Debug, Serialize)]
    pub struct Export {
        pub name: String,
        pub rva: u32,
        pub prefix: [u8; 8],
    }

    #[derive(Debug, Serialize)]
    pub struct PeImage {
        pub size: u32,
        pub fields: Vec<ImageField>,
        pub exports: Vec<Export>,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct VerifiedAttachExports {
        breakpoint_address: u64,
        breakin_address: u64,
    }

    impl VerifiedAttachExports {
        pub fn breakpoint_address(&self) -> u64 {
            self.breakpoint_address
        }
        pub fn breakin_address(&self) -> u64 {
            self.breakin_address
        }
    }

    #[derive(Debug)]
    struct Section {
        va: u32,
        virtual_size: u32,
        raw: u32,
        raw_size: u32,
        flags: u32,
    }

    fn bytes_at(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
        bytes
            .get(offset..offset.checked_add(length).ok_or("PE field overflow")?)
            .ok_or_else(|| "PE field outside bounded file".into())
    }
    fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(
            bytes_at(bytes, offset, 2)?
                .try_into()
                .map_err(|_| "PE u16 size")?,
        ))
    }
    fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(
            bytes_at(bytes, offset, 4)?
                .try_into()
                .map_err(|_| "PE u32 size")?,
        ))
    }

    fn mapped_offset(sections: &[Section], headers: u32, rva: u32, length: usize) -> Result<usize> {
        let end = u64::from(rva)
            .checked_add(length as u64)
            .ok_or("PE RVA overflow")?;
        if end <= u64::from(headers) {
            return Ok(rva as usize);
        }
        let matches: Vec<_> = sections
            .iter()
            .filter(|section| {
                rva >= section.va
                    && end
                        <= u64::from(section.va)
                            + u64::from(section.virtual_size.max(section.raw_size))
            })
            .collect();
        if matches.len() != 1 {
            return Err("PE RVA has no unique mapped section".into());
        }
        let section = matches[0];
        let delta = rva - section.va;
        if u64::from(delta) + length as u64 > u64::from(section.raw_size) {
            return Err("PE RVA is not backed by file bytes".into());
        }
        (section.raw as usize)
            .checked_add(delta as usize)
            .ok_or_else(|| "PE file offset overflow".into())
    }

    fn add_field(
        fields: &mut Vec<ImageField>,
        bytes: &[u8],
        rva: u32,
        offset: usize,
        length: usize,
    ) -> Result<()> {
        let value = bytes_at(bytes, offset, length)?;
        for (index, chunk) in value.chunks(8).enumerate() {
            fields.push(ImageField {
                rva: rva
                    .checked_add((index * 8) as u32)
                    .ok_or("PE field RVA overflow")?,
                bytes: chunk.to_vec(),
            });
        }
        Ok(())
    }

    impl PeImage {
        pub fn parse(bytes: &[u8]) -> Result<Self> {
            if bytes.len() > MAX_FILE_BYTES || u16_at(bytes, 0)? != 0x5A4D {
                return Err("unsupported DOS header/file size".into());
            }
            let pe = u32_at(bytes, 0x3C)? as usize;
            if !(64..=65536).contains(&pe)
                || u32_at(bytes, pe)? != 0x4550
                || u16_at(bytes, pe + 4)? != 0x8664
            {
                return Err("not a native AMD64 PE image".into());
            }
            let count = u16_at(bytes, pe + 6)? as usize;
            let opt_size = u16_at(bytes, pe + 20)? as usize;
            let opt = pe + 24;
            if !(1..=96).contains(&count)
                || !(240..=512).contains(&opt_size)
                || u16_at(bytes, pe + 22)? & 0x2000 == 0
                || u16_at(bytes, opt)? != 0x20B
            {
                return Err("unsupported PE64 DLL/section header".into());
            }
            bytes_at(bytes, opt, opt_size)?;
            let size = u32_at(bytes, opt + 56)?;
            let headers = u32_at(bytes, opt + 60)?;
            let table = opt + opt_size;
            if size == 0
                || size > 64 * 1024 * 1024
                || headers as usize > bytes.len()
                || table + count * 40 > headers as usize
                || u32_at(bytes, opt + 108)? == 0
            {
                return Err("invalid PE image/header/data-directory bounds".into());
            }
            let export_rva = u32_at(bytes, opt + 112)?;
            let export_size = u32_at(bytes, opt + 116)?;
            if export_rva == 0
                || export_size < 40
                || u64::from(export_rva) + u64::from(export_size) > u64::from(size)
            {
                return Err("invalid PE export directory".into());
            }
            let mut sections = Vec::new();
            let mut fields = Vec::new();
            for (offset, length) in [
                (0, 2),
                (0x3C, 4),
                (pe, 4),
                (pe + 4, 2),
                (pe + 6, 2),
                (pe + 8, 4),
                (pe + 20, 2),
                (pe + 22, 2),
                (opt, 2),
                (opt + 56, 4),
                (opt + 60, 4),
                (opt + 64, 4),
                (opt + 108, 4),
                (opt + 112, 8),
            ] {
                add_field(&mut fields, bytes, offset as u32, offset, length)?;
            }
            for index in 0..count {
                let offset = table + index * 40;
                let section = Section {
                    virtual_size: u32_at(bytes, offset + 8)?,
                    va: u32_at(bytes, offset + 12)?,
                    raw_size: u32_at(bytes, offset + 16)?,
                    raw: u32_at(bytes, offset + 20)?,
                    flags: u32_at(bytes, offset + 36)?,
                };
                if u64::from(section.va) + u64::from(section.virtual_size.max(section.raw_size))
                    > u64::from(size)
                {
                    return Err("PE section exceeds image".into());
                }
                bytes_at(bytes, section.raw as usize, section.raw_size as usize)?;
                for field in [8, 12, 16, 20, 36] {
                    add_field(
                        &mut fields,
                        bytes,
                        (offset + field) as u32,
                        offset + field,
                        4,
                    )?;
                }
                sections.push(section);
            }
            let exp = mapped_offset(&sections, headers, export_rva, 40)?;
            add_field(&mut fields, bytes, export_rva, exp, 40)?;
            let functions_count = u32_at(bytes, exp + 20)?;
            let names_count = u32_at(bytes, exp + 24)?;
            if functions_count == 0 || functions_count > 65536 || names_count > 65536 {
                return Err("PE export tables exceed finite bounds".into());
            }
            let functions_rva = u32_at(bytes, exp + 28)?;
            let names_rva = u32_at(bytes, exp + 32)?;
            let ordinals_rva = u32_at(bytes, exp + 36)?;
            let functions = mapped_offset(
                &sections,
                headers,
                functions_rva,
                functions_count as usize * 4,
            )?;
            let names = mapped_offset(&sections, headers, names_rva, names_count as usize * 4)?;
            let ordinals =
                mapped_offset(&sections, headers, ordinals_rva, names_count as usize * 2)?;
            bytes_at(bytes, functions, functions_count as usize * 4)?;
            bytes_at(bytes, names, names_count as usize * 4)?;
            bytes_at(bytes, ordinals, names_count as usize * 2)?;
            let mut exports = Vec::new();
            for index in 0..names_count as usize {
                let name_rva = u32_at(bytes, names + index * 4)?;
                let mut name = Vec::new();
                let mut terminated = false;
                for delta in 0..=127 {
                    let rva = name_rva
                        .checked_add(delta)
                        .ok_or("export name RVA overflow")?;
                    let offset = mapped_offset(&sections, headers, rva, 1)?;
                    let byte = bytes_at(bytes, offset, 1)?[0];
                    if byte == 0 {
                        terminated = true;
                        break;
                    }
                    name.push(byte);
                }
                if !terminated {
                    return Err("export name exceeds bounded length".into());
                }
                let Some(wanted) = EXPORT_NAMES.iter().find(|wanted| wanted.as_bytes() == name)
                else {
                    continue;
                };
                if exports.iter().any(|export: &Export| export.name == *wanted) {
                    return Err("duplicate named export".into());
                }
                let ordinal = u16_at(bytes, ordinals + index * 2)? as usize;
                if ordinal >= functions_count as usize {
                    return Err("export ordinal exceeds function table".into());
                }
                let rva = u32_at(bytes, functions + ordinal * 4)?;
                if u64::from(rva) >= u64::from(export_rva)
                    && u64::from(rva) < u64::from(export_rva) + u64::from(export_size)
                {
                    return Err("forwarded export is not an attach entry".into());
                }
                let offset = mapped_offset(&sections, headers, rva, 8)?;
                if !sections.iter().any(|section| {
                    section.flags & 0x20000000 != 0
                        && rva >= section.va
                        && u64::from(rva) + 8
                            <= u64::from(section.va) + u64::from(section.virtual_size)
                }) {
                    return Err("attach export is outside executable section".into());
                }
                let prefix: [u8; 8] = bytes_at(bytes, offset, 8)?
                    .try_into()
                    .map_err(|_| "export prefix size")?;
                if *wanted == "DbgBreakPoint" && prefix[..2] != [0xCC, 0xC3] {
                    return Err("DbgBreakPoint is not the expected INT3/RET stub".into());
                }
                for (field_rva, field_offset, length) in [
                    (
                        functions_rva + ordinal as u32 * 4,
                        functions + ordinal * 4,
                        4,
                    ),
                    (names_rva + index as u32 * 4, names + index * 4, 4),
                    (ordinals_rva + index as u32 * 2, ordinals + index * 2, 2),
                ] {
                    add_field(&mut fields, bytes, field_rva, field_offset, length)?;
                }
                let name_offset = mapped_offset(&sections, headers, name_rva, name.len() + 1)?;
                add_field(&mut fields, bytes, name_rva, name_offset, name.len() + 1)?;
                exports.push(Export {
                    name: (*wanted).into(),
                    rva,
                    prefix,
                });
            }
            if exports.len() != EXPORT_NAMES.len() {
                return Err("required direct ntdll export is missing".into());
            }
            Ok(Self {
                size,
                fields,
                exports,
            })
        }

        pub fn export(&self, name: &str) -> Result<&Export> {
            self.exports
                .iter()
                .find(|export| export.name == name)
                .ok_or_else(|| "missing audited export".into())
        }

        pub fn verify_loaded(
            &self,
            reader: &impl ShortImageReader,
            base: u64,
            size: u32,
        ) -> Result<VerifiedAttachExports> {
            if base == 0 || size != self.size {
                return Err("loaded ntdll ASLR base/image size mismatch".into());
            }
            for field in &self.fields {
                if field.bytes.is_empty()
                    || field.bytes.len() > 8
                    || u64::from(field.rva) + field.bytes.len() as u64 > u64::from(size)
                {
                    return Err("invalid short PE identity field".into());
                }
                let address = base
                    .checked_add(u64::from(field.rva))
                    .ok_or("loaded PE address overflow")?;
                if reader.read_field(address, field.bytes.len())? != field.bytes {
                    return Err(format!(
                        "loaded ntdll PE/export field mismatch at RVA {:#x}",
                        field.rva
                    ));
                }
            }
            for export in &self.exports {
                let address = base
                    .checked_add(u64::from(export.rva))
                    .ok_or("loaded export address overflow")?;
                if reader.read_field(address, 8)? != export.prefix {
                    return Err(format!(
                        "loaded ntdll export prefix mismatch: {}",
                        export.name
                    ));
                }
            }
            Ok(VerifiedAttachExports {
                breakpoint_address: base
                    .checked_add(u64::from(self.export("DbgBreakPoint")?.rva))
                    .ok_or("breakpoint address overflow")?,
                breakin_address: base
                    .checked_add(u64::from(self.export("DbgUiRemoteBreakin")?.rva))
                    .ok_or("break-in address overflow")?,
            })
        }
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
mod platform {
    use super::{attach_image::*, policy::*, *};
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{
        collections::{BTreeMap, BTreeSet},
        ffi::{c_void, OsString},
        fs::{File, OpenOptions},
        io::{Read, Write},
        mem::size_of,
        os::windows::{ffi::OsStringExt, fs::OpenOptionsExt, io::AsRawHandle},
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, FreeLibrary, GetLastError, DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED,
            ERROR_NO_MORE_FILES, ERROR_SEM_TIMEOUT, EXCEPTION_SINGLE_STEP, FILETIME, HANDLE,
            HMODULE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        Storage::FileSystem::{GetFinalPathNameByHandleW, FILE_SHARE_READ},
        System::{
            Diagnostics::{
                Debug::{
                    ContinueDebugEvent, DebugActiveProcess, DebugActiveProcessStop,
                    DebugSetProcessKillOnExit, GetThreadContext, ReadProcessMemory,
                    SetThreadContext, WaitForDebugEvent, CONTEXT, CONTEXT_CONTROL_AMD64,
                    CONTEXT_DEBUG_REGISTERS_AMD64, CONTEXT_INTEGER_AMD64,
                    CREATE_PROCESS_DEBUG_EVENT, CREATE_THREAD_DEBUG_EVENT, DEBUG_EVENT,
                    EXCEPTION_DEBUG_EVENT, EXIT_PROCESS_DEBUG_EVENT, EXIT_THREAD_DEBUG_EVENT,
                    LOAD_DLL_DEBUG_EVENT, RIP_EVENT, UNLOAD_DLL_DEBUG_EVENT,
                },
                ToolHelp::{
                    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Thread32First,
                    Thread32Next, MODULEENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPTHREAD,
                    THREADENTRY32,
                },
            },
            LibraryLoader::{
                GetModuleFileNameW, GetModuleHandleExW, GetModuleHandleW, GetProcAddress,
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            },
            SystemInformation::GetSystemDirectoryW,
            Threading::{
                GetProcessId, GetProcessIdOfThread, GetProcessTimes, IsWow64Process, OpenProcess,
                OpenThread, QueryFullProcessImageNameW, ResumeThread, SuspendThread,
                WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
                PROCESS_VM_READ, THREAD_GET_CONTEXT, THREAD_QUERY_INFORMATION,
                THREAD_QUERY_LIMITED_INFORMATION, THREAD_SET_CONTEXT, THREAD_SUSPEND_RESUME,
                THREAD_SYNCHRONIZE,
            },
        },
    };

    fn win_error(operation: &str) -> String {
        // SAFETY: GetLastError only reads this caller's thread-local status.
        let code = unsafe { GetLastError() };
        format!("{operation}: Windows error {code}")
    }

    #[derive(Debug)]
    struct Handle(HANDLE);

    impl Handle {
        fn new(raw: HANDLE, operation: &str) -> Result<Self> {
            if raw.is_null() || raw == INVALID_HANDLE_VALUE {
                Err(win_error(operation))
            } else {
                Ok(Self(raw))
            }
        }

        // ContinueDebugEvent automatically closes these original debug-event
        // process/thread handles when their EXIT event is continued.
        fn closed_by_continue(mut self) {
            self.0 = std::ptr::null_mut();
        }

        fn alive(&self) -> Result<bool> {
            // SAFETY: the handle is owned and has synchronization access.
            match unsafe { WaitForSingleObject(self.0, 0) } {
                WAIT_TIMEOUT => Ok(true),
                WAIT_OBJECT_0 => Ok(false),
                _ => Err(win_error("WaitForSingleObject")),
            }
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: owned handle, or original event handle whose EXIT
                // has not been continued; it is closed exactly once.
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    fn canonical(path: &Path) -> Result<PathBuf> {
        std::fs::canonicalize(path).map_err(|e| format!("canonicalize pinned path: {e}"))
    }

    fn path_key(path: &Path) -> Result<String> {
        path.to_str()
            .map(|s| s.to_lowercase())
            .ok_or_else(|| "pinned path is not valid Unicode".into())
    }

    fn wide_path(value: &[u16]) -> Result<PathBuf> {
        let end = value
            .iter()
            .position(|&c| c == 0)
            .ok_or("truncated Toolhelp path")?;
        Ok(PathBuf::from(OsString::from_wide(&value[..end])))
    }

    fn file_path(raw: HANDLE) -> Result<PathBuf> {
        let mut buffer = vec![0u16; 32768];
        // SAFETY: borrowed valid file handle, bounded UTF-16 destination.
        let len =
            unsafe { GetFinalPathNameByHandleW(raw, buffer.as_mut_ptr(), buffer.len() as u32, 0) };
        if len == 0 {
            return Err(win_error("GetFinalPathNameByHandleW"));
        }
        if len as usize >= buffer.len() {
            return Err("file path exceeds bounded buffer".into());
        }
        canonical(&PathBuf::from(OsString::from_wide(&buffer[..len as usize])))
    }

    fn identity(raw: HANDLE) -> Result<Identity> {
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        // SAFETY: borrowed process handle and valid FILETIME destinations.
        if unsafe { GetProcessTimes(raw, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
            return Err(win_error("GetProcessTimes"));
        }
        let mut buffer = vec![0u16; 32768];
        let mut len = buffer.len() as u32;
        // SAFETY: same borrowed process handle, bounded UTF-16 destination.
        if unsafe { QueryFullProcessImageNameW(raw, 0, buffer.as_mut_ptr(), &mut len) } == 0 {
            return Err(win_error("QueryFullProcessImageNameW"));
        }
        let image = canonical(&PathBuf::from(OsString::from_wide(&buffer[..len as usize])))?;
        // SAFETY: same process handle; no PID is opened a second time.
        let pid = unsafe { GetProcessId(raw) };
        if pid == 0 {
            return Err(win_error("GetProcessId"));
        }
        Ok(Identity {
            pid,
            created: (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime),
            image_path: path_key(&image)?,
        })
    }

    struct Process {
        handle: Handle,
        pinned: Identity,
    }

    impl Process {
        fn open(args: &Args) -> Result<Self> {
            // SAFETY: only the specified PID, read/query/synchronize access;
            // deliberately no VM_WRITE, VM_OPERATION, or injection rights.
            let handle = Handle::new(
                unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ | PROCESS_SYNCHRONIZE,
                        0,
                        args.pid,
                    )
                },
                "OpenProcess",
            )?;
            if !handle.alive()? {
                return Err("specified process has exited".into());
            }
            let pinned = identity(handle.0)?;
            let expected = Identity {
                pid: args.pid,
                created: args.created,
                image_path: pinned.image_path.clone(),
            };
            match_identity(&expected, &pinned, handle.alive()?)?;
            expected_module(Path::new(&pinned.image_path))?;
            let mut wow64 = 0;
            // SAFETY: owned process handle and valid BOOL destination.
            if unsafe { IsWow64Process(handle.0, &mut wow64) } == 0 {
                return Err(win_error("IsWow64Process"));
            }
            if wow64 != 0 {
                return Err("target is not a native x64 process".into());
            }
            Ok(Self { handle, pinned })
        }

        fn verify(&self) -> Result<()> {
            match_identity(
                &self.pinned,
                &identity(self.handle.0)?,
                self.handle.alive()?,
            )
        }

        fn read<const N: usize>(&self, address: u64) -> Result<[u8; N]> {
            self.read_short(address, N)?
                .try_into()
                .map_err(|_| "short fixed field size mismatch".into())
        }

        fn read_short(&self, address: u64, length: usize) -> Result<Vec<u8>> {
            if !(1..=8).contains(&length)
                || address == 0
                || address.checked_add(length as u64).is_none()
            {
                return Err("invalid bounded field read".into());
            }
            let mut bytes = [0; 8];
            let mut received = 0;
            // SAFETY: Windows validates the remote address; local destination
            // is at least length bytes. This boundary accepts only fields <= 8 bytes.
            if unsafe {
                ReadProcessMemory(
                    self.handle.0,
                    address as *const c_void,
                    bytes.as_mut_ptr().cast(),
                    length,
                    &mut received,
                )
            } == 0
            {
                return Err(win_error("ReadProcessMemory(field <= 8 bytes)"));
            }
            if received != length {
                return Err("short remote field read".into());
            }
            Ok(bytes[..length].to_vec())
        }

        fn at(base: u64, offset: u64) -> Result<u64> {
            if base == 0 {
                return Err("null field base".into());
            }
            base.checked_add(offset)
                .ok_or_else(|| "field address overflow".into())
        }

        fn u64_at(&self, base: u64, offset: u64) -> Result<u64> {
            Ok(u64::from_le_bytes(self.read::<8>(Self::at(base, offset)?)?))
        }

        fn i32_at(&self, base: u64, offset: u64) -> Result<i32> {
            Ok(i32::from_le_bytes(self.read::<4>(Self::at(base, offset)?)?))
        }

        fn u16_at(&self, base: u64, offset: u64) -> Result<u16> {
            Ok(u16::from_le_bytes(self.read::<2>(Self::at(base, offset)?)?))
        }

        fn interface_fields(&self, module_base: u64) -> Value {
            // The pinned DLL's 0x45089 stores an externally supplied interface
            // at this RVA; 0x72988 calls its vtable+0xB0. Read only those three
            // fields, without identifying the interface or calling its slot.
            let pointer = self.u64_at(module_base, 0xB8AA80);
            let vtable = match &pointer {
                Ok(pointer) if *pointer != 0 => Some(self.u64_at(*pointer, 0)),
                _ => None,
            };
            let slot = match &vtable {
                Some(Ok(vtable)) if *vtable != 0 => Some(self.u64_at(*vtable, 0xB0)),
                _ => None,
            };
            json!({"raw_module_plus_b8aa80_pointer":raw_read(pointer),
                "raw_interface_vtable_pointer":vtable.map(raw_read),
                "raw_vtable_plus_b0_pointer":slot.map(raw_read),
                "semantics":"unidentified_external_interface_slot_not_called"})
        }

        fn main_image_fields(&self) -> Value {
            let name = Path::new(&self.pinned.image_path)
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| "pinned image has no Unicode file name".to_owned());
            raw_read(name.and_then(|name| find_named_module(self, &self.pinned.image_path, name)))
        }

        fn cache_fields(&self, trigger: Trigger) -> Value {
            let pointer = self.u64_at(trigger.cache_city, 0x16C0);
            let raw_b8 = self.u64_at(trigger.receiver, 0xB8);
            let raw_b8_fields = match &raw_b8 {
                Ok(0) => Value::Null,
                Ok(pointer) => json!({
                    "raw_pointer_plus_38_u64": raw_read(self.u64_at(*pointer, 0x38)),
                    "raw_pointer_plus_40_u64": raw_read(self.u64_at(*pointer, 0x40)),
                }),
                Err(error) => json!({"read_error":format!("pointee unavailable: {error}")}),
            };
            let backpointer = self.u64_at(trigger.receiver, 0x90);
            // Same address is read through both supplied layouts so disagreement
            // is visible; neither pointer is named as a proven semantic object.
            let pointee = match &pointer {
                Ok(0) => Value::Null,
                Ok(pointer) => {
                    json!({"raw_pointer_plus_a8_i32": raw_read(self.i32_at(*pointer, 0xA8)),
                    "raw_pointer_plus_d8_u16": raw_read(self.u16_at(*pointer, 0xD8))})
                }
                Err(error) => json!({"read_error":format!("pointee unavailable: {error}")}),
            };
            json!({
                "raw_cache_plus_16d8_i32": raw_read(self.i32_at(trigger.cache_city, 0x16D8)),
                "raw_cache_plus_16f0_i32": raw_read(self.i32_at(trigger.cache_city, 0x16F0)),
                "raw_cache_plus_16c0_pointer": raw_read(pointer),
                "raw_receiver_plus_90_pointer": raw_read(backpointer),
                "raw_receiver_plus_a8_i32": raw_read(self.i32_at(trigger.receiver, 0xA8)),
                "raw_receiver_plus_b8_pointer": raw_read(raw_b8),
                "raw_cache_plus_16e8_pointer": raw_read(self.u64_at(trigger.cache_city, 0x16E8)),
                "raw_b8_pointee_fields": raw_b8_fields,
                "raw_pointee_fields": pointee,
            })
        }
    }

    impl ShortImageReader for Process {
        fn read_field(&self, address: u64, length: usize) -> Result<Vec<u8>> {
            self.read_short(address, length)
        }
    }

    #[derive(Debug, PartialEq, Eq, Serialize)]
    struct ModuleIdentity {
        path: String,
        base: u64,
        size: u32,
    }

    fn find_module(process: &Process) -> Result<ModuleIdentity> {
        process.verify()?;
        // SAFETY: metadata snapshot for the pinned PID; no heap/memory dump.
        let snapshot = Handle::new(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, process.pinned.pid) },
            "CreateToolhelp32Snapshot(modules)",
        )?;
        let mut entry = MODULEENTRY32W {
            dwSize: size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        // SAFETY: valid snapshot and correctly sized destination.
        if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
            return Err(win_error("Module32FirstW"));
        }
        let expected_path = path_key(&canonical(&expected_module(Path::new(
            &process.pinned.image_path,
        ))?)?)?;
        let mut found = None;
        loop {
            let name = wide_path(&entry.szModule)?;
            let name = name.to_str().ok_or("module name is not Unicode")?;
            if name.to_ascii_lowercase().starts_with("gamecore_") {
                if !name.eq_ignore_ascii_case(MODULE_NAME) {
                    return Err(
                        "another GameCore module is loaded; refusing unsupported DLL".into(),
                    );
                }
                let module = ModuleIdentity {
                    path: path_key(&canonical(&wide_path(&entry.szExePath)?)?)?,
                    base: entry.modBaseAddr as u64,
                    size: entry.modBaseSize,
                };
                if found.is_some()
                    || module.path != expected_path
                    || entry.th32ProcessID != process.pinned.pid
                {
                    return Err("duplicate module or module path/PID mismatch".into());
                }
                breakpoint_addresses(module.base, module.size)?;
                found = Some(module);
            }
            // SAFETY: same metadata snapshot and sized destination.
            if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
                // SAFETY: capture this API's error before another call.
                if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                    return Err(win_error("Module32NextW"));
                }
                break;
            }
        }
        process.verify()?;
        found.ok_or_else(|| "specified XP2 module is not loaded".into())
    }

    fn system_ntdll_path() -> Result<String> {
        let mut buffer = vec![0u16; 32768];
        // SAFETY: bounded writable UTF-16 buffer; native x64 system directory.
        let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
        if length == 0 {
            return Err(win_error("GetSystemDirectoryW"));
        }
        if length as usize >= buffer.len() {
            return Err("system directory path exceeds buffer".into());
        }
        path_key(&canonical(
            &PathBuf::from(OsString::from_wide(&buffer[..length as usize])).join("ntdll.dll"),
        )?)
    }

    fn find_named_module(
        process: &Process,
        expected_path: &str,
        expected_name: &str,
    ) -> Result<ModuleIdentity> {
        process.verify()?;
        // SAFETY: metadata snapshot only for the already pinned PID.
        let snapshot = Handle::new(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, process.pinned.pid) },
            "named module snapshot",
        )?;
        let mut entry = MODULEENTRY32W {
            dwSize: size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        // SAFETY: valid snapshot and sized metadata destination.
        if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
            return Err(win_error("Module32FirstW(named module)"));
        }
        let mut found = None;
        loop {
            if wide_path(&entry.szModule)?
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case(expected_name))
            {
                let image = ModuleIdentity {
                    path: path_key(&canonical(&wide_path(&entry.szExePath)?)?)?,
                    base: entry.modBaseAddr as u64,
                    size: entry.modBaseSize,
                };
                if found.is_some()
                    || image.path != expected_path
                    || entry.th32ProcessID != process.pinned.pid
                {
                    return Err("named module path/PID/unique-image mismatch".into());
                }
                found = Some(image);
            }
            // SAFETY: same snapshot and destination.
            if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
                // SAFETY: inspect the enumeration status immediately.
                if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                    return Err(win_error("Module32NextW(named module)"));
                }
                break;
            }
        }
        process.verify()?;
        found.ok_or_else(|| "specified image is not loaded in the pinned process".into())
    }

    type NtQueryThread = unsafe extern "system" fn(HANDLE, i32, *mut c_void, u32, *mut u32) -> i32;

    struct LocalThreadQuery {
        module: HMODULE,
        function: NtQueryThread,
    }

    impl LocalThreadQuery {
        fn bind(expected_path: &str, image: &PeImage) -> Result<Self> {
            let name: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
            // SAFETY: borrow already loaded local ntdll, never load a target DLL.
            let local = unsafe { GetModuleHandleW(name.as_ptr()) };
            if local.is_null() {
                return Err(win_error("GetModuleHandleW(ntdll)"));
            }
            // SAFETY: local module and constant nul-terminated export name.
            let function =
                unsafe { GetProcAddress(local, c"NtQueryInformationThread".as_ptr().cast()) }
                    .ok_or("local NtQueryInformationThread export unavailable")?;
            let address = function as usize as u64;
            let expected = (local as u64)
                .checked_add(u64::from(image.export("NtQueryInformationThread")?.rva))
                .ok_or("local NtQuery RVA overflow")?;
            if address != expected {
                return Err("local NtQueryInformationThread was forwarded or moved".into());
            }
            let mut owner = std::ptr::null_mut();
            // SAFETY: FROM_ADDRESS uses the address rather than dereferencing a
            // string; take one reference to the actual owning local image.
            if unsafe {
                GetModuleHandleExW(
                    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                    address as *const u16,
                    &mut owner,
                )
            } == 0
            {
                return Err(win_error("GetModuleHandleExW(NtQuery owner)"));
            }
            struct Library(HMODULE);
            impl Drop for Library {
                fn drop(&mut self) {
                    /* SAFETY: owned local library reference. */
                    unsafe {
                        FreeLibrary(self.0);
                    }
                }
            }
            let owner = Library(owner);
            if owner.0 != local {
                return Err("NtQueryInformationThread belongs to another local image".into());
            }
            let mut path = vec![0u16; 32768];
            // SAFETY: held library reference and bounded filename destination.
            let length =
                unsafe { GetModuleFileNameW(owner.0, path.as_mut_ptr(), path.len() as u32) };
            if length == 0 {
                return Err(win_error("GetModuleFileNameW(NtQuery owner)"));
            }
            if length as usize >= path.len() {
                return Err("local ntdll path exceeds buffer".into());
            }
            if path_key(&canonical(&PathBuf::from(OsString::from_wide(
                &path[..length as usize],
            )))?)?
                != expected_path
            {
                return Err("local/remote ntdll file path mismatch".into());
            }
            // SAFETY: direct named export in its held system ntdll image. ABI
            // follows NtQueryInformationThread. NEVER used as a remote address.
            let function = unsafe {
                std::mem::transmute::<unsafe extern "system" fn() -> isize, NtQueryThread>(function)
            };
            let module = owner.0;
            std::mem::forget(owner);
            Ok(Self { module, function })
        }

        fn start_address(&self, handle: &Handle) -> Result<u64> {
            let mut start: usize = 0;
            let mut length = 0;
            // SAFETY: owned target thread with QUERY_INFORMATION, documented
            // class 9, exact pointer-sized output; query only, no context write.
            let status = unsafe {
                (self.function)(
                    handle.0,
                    9,
                    (&mut start as *mut usize).cast(),
                    size_of::<usize>() as u32,
                    &mut length,
                )
            };
            if status < 0 {
                return Err(format!(
                    "NtQueryInformationThread(start): NTSTATUS {:#x}",
                    status as u32
                ));
            }
            if length != size_of::<usize>() as u32 || start == 0 {
                return Err("OS thread start query returned invalid length/address".into());
            }
            Ok(start as u64)
        }
    }

    impl Drop for LocalThreadQuery {
        fn drop(&mut self) {
            /* SAFETY: owned local module reference; query calls have finished. */
            unsafe {
                FreeLibrary(self.module);
            }
        }
    }

    struct PinnedNtdll {
        identity: ModuleIdentity,
        image: PeImage,
        sha256: String,
        query: LocalThreadQuery,
        _file: File,
    }

    impl PinnedNtdll {
        fn open(process: &Process) -> Result<Self> {
            let path = system_ntdll_path()?;
            let identity = find_named_module(process, &path, "ntdll.dll")?;
            let mut file = OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(&path)
                .map_err(|error| format!("open read-only system ntdll: {error}"))?;
            if path_key(&file_path(file.as_raw_handle())?)? != path {
                return Err("ntdll file-handle identity mismatch".into());
            }
            let mut bytes = Vec::new();
            (&mut file)
                .take((MAX_FILE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("read bounded ntdll file: {error}"))?;
            if bytes.len() > MAX_FILE_BYTES {
                return Err("ntdll file exceeds finite size limit".into());
            }
            let sha256 = format!("{:X}", Sha256::digest(&bytes));
            let image = PeImage::parse(&bytes)?;
            image.verify_loaded(process, identity.base, identity.size)?;
            let query = LocalThreadQuery::bind(&path, &image)?;
            Ok(Self {
                identity,
                image,
                sha256,
                query,
                _file: file,
            })
        }

        fn verify(&self, process: &Process) -> Result<VerifiedAttachExports> {
            if find_named_module(process, &self.identity.path, "ntdll.dll")? != self.identity {
                return Err("pinned ntdll module identity changed".into());
            }
            self.image
                .verify_loaded(process, self.identity.base, self.identity.size)
        }

        fn report(&self) -> Value {
            json!({"module":self.identity,"file_sha256":self.sha256,
            "exports":self.image.exports,"checked_remote_fields":self.image.fields.len(),
            "scope":"PE_metadata_and_eight_byte_export_prefixes"})
        }
    }

    struct PinnedModule {
        identity: ModuleIdentity,
        // Deny write/delete sharing throughout the session to pin the hashed
        // on-disk module. This SHA is not a hash of all loaded process memory.
        _file: File,
        addresses: [u64; 4],
    }

    impl PinnedModule {
        fn open(process: &Process) -> Result<Self> {
            let identity = find_module(process)?;
            let mut file = OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(&identity.path)
                .map_err(|e| format!("open pinned DLL read-only: {e}"))?;
            if path_key(&file_path(file.as_raw_handle())?)? != identity.path {
                return Err("module file handle path mismatch".into());
            }
            let mut hasher = Sha256::new();
            let mut bytes = [0; 65536];
            loop {
                let n = file
                    .read(&mut bytes)
                    .map_err(|e| format!("hash pinned DLL file: {e}"))?;
                if n == 0 {
                    break;
                }
                hasher.update(&bytes[..n]);
            }
            let sha = format!("{:X}", hasher.finalize());
            if sha != MODULE_SHA256 {
                return Err(format!("module file SHA256 mismatch: {sha}"));
            }
            let addresses = breakpoint_addresses(identity.base, identity.size)?;
            let module = Self {
                identity,
                _file: file,
                addresses,
            };
            module.verify(process)?;
            Ok(module)
        }

        fn verify(&self, process: &Process) -> Result<()> {
            if find_module(process)? != self.identity {
                return Err("pinned module path, ASLR base, or size changed".into());
            }
            Ok(())
        }
    }

    fn context(handle: &Handle, flags: u32) -> Result<CONTEXT> {
        // Windows CONTEXT requires 16-byte alignment, which windows-sys' C
        // layout alone does not express on every compiler/target.
        #[repr(C, align(16))]
        struct Aligned(CONTEXT);
        let mut value = Aligned(CONTEXT {
            ContextFlags: flags,
            ..Default::default()
        });
        // SAFETY: owned thread is stopped by a debug event or SuspendThread;
        // destination has the required alignment and requested register groups.
        if unsafe { GetThreadContext(handle.0, &mut value.0) } == 0 {
            return Err(win_error("GetThreadContext"));
        }
        Ok(value.0)
    }

    fn set_context(handle: &Handle, value: CONTEXT) -> Result<()> {
        #[repr(C, align(16))]
        struct Aligned(CONTEXT);
        let value = Aligned(value);
        // SAFETY: owned stopped thread; aligned CONTEXT. Callers select only
        // debug/control groups, preserve control registers, and only add RF.
        if unsafe { SetThreadContext(handle.0, &value.0) } == 0 {
            Err(win_error("SetThreadContext"))
        } else {
            Ok(())
        }
    }

    fn registers(value: &CONTEXT) -> DebugRegisters {
        DebugRegisters {
            address: [value.Dr0, value.Dr1, value.Dr2, value.Dr3],
            dr6: value.Dr6,
            dr7: value.Dr7,
        }
    }

    fn replace_registers(value: &mut CONTEXT, registers: DebugRegisters) {
        [value.Dr0, value.Dr1, value.Dr2, value.Dr3] = registers.address;
        value.Dr6 = registers.dr6;
        value.Dr7 = registers.dr7;
    }

    struct Suspension<'a> {
        handle: &'a Handle,
        resumed: bool,
    }

    impl<'a> Suspension<'a> {
        fn new(handle: &'a Handle) -> Result<Self> {
            // SAFETY: owned target thread, one reversible suspend-count increment.
            if unsafe { SuspendThread(handle.0) } == u32::MAX {
                return Err(win_error("SuspendThread"));
            }
            Ok(Self {
                handle,
                resumed: false,
            })
        }

        fn resume(&mut self) -> Result<()> {
            if self.resumed {
                return Ok(());
            }
            // SAFETY: exactly one increment owned by this guard is removed.
            if unsafe { ResumeThread(self.handle.0) } == u32::MAX {
                return Err(win_error("ResumeThread"));
            }
            self.resumed = true;
            Ok(())
        }
    }

    impl Drop for Suspension<'_> {
        fn drop(&mut self) {
            if !self.resumed {
                let _ = self.resume();
            }
        }
    }

    fn preflight_threads(process: &Process) -> Result<BTreeSet<u32>> {
        // SAFETY: thread metadata only; filter ownership before opening handles.
        let snapshot = Handle::new(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) },
            "thread snapshot",
        )?;
        let mut entry = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        // SAFETY: valid snapshot and correctly sized entry.
        if unsafe { Thread32First(snapshot.0, &mut entry) } == 0 {
            return Err(win_error("Thread32First"));
        }
        let mut count = 0;
        let mut tids = BTreeSet::new();
        loop {
            if entry.th32OwnerProcessID == process.pinned.pid {
                // SAFETY: only a thread reported as owned by the pinned PID.
                let thread = Handle::new(
                    unsafe {
                        OpenThread(
                            THREAD_GET_CONTEXT
                                | THREAD_SUSPEND_RESUME
                                | THREAD_QUERY_LIMITED_INFORMATION
                                | THREAD_SYNCHRONIZE,
                            0,
                            entry.th32ThreadID,
                        )
                    },
                    "OpenThread(preflight)",
                )?;
                // SAFETY: verify the same handle before any suspension; TID reuse
                // must never lead to touching an unrelated process.
                if unsafe { GetProcessIdOfThread(thread.0) } != process.pinned.pid {
                    return Err("preflight thread owner mismatch".into());
                }
                process.verify()?;
                let mut suspension = Suspension::new(&thread)?;
                let observed = context(&thread, CONTEXT_DEBUG_REGISTERS_AMD64);
                suspension.resume()?;
                let observed = observed?;
                if !registers(&observed).unused() {
                    return Err(format!(
                        "pre-existing debug registers on thread {}",
                        entry.th32ThreadID
                    ));
                }
                count += 1;
                tids.insert(entry.th32ThreadID);
            }
            // SAFETY: same snapshot and sized entry.
            if unsafe { Thread32Next(snapshot.0, &mut entry) } == 0 {
                // SAFETY: capture enumeration error immediately.
                if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                    return Err(win_error("Thread32Next"));
                }
                break;
            }
        }
        if count == 0 {
            return Err("pinned process has no accessible threads".into());
        }
        process.verify()?;
        Ok(tids)
    }

    struct Thread {
        handle: Handle,
        original: DebugRegisters,
        armed: bool,
        enabled: u64,
        suspended: bool,
        suspend_previous_count: Option<u32>,
        cleanup_snapshot: Option<ProbeSnapshot>,
    }

    struct Pending {
        event: DEBUG_EVENT,
        status: i32,
        restore_before_continue: bool,
        exception_routed: bool,
        decision: Option<ExceptionDecision>,
    }

    struct Session {
        process: Process,
        module: PinnedModule,
        ntdll: PinnedNtdll,
        preexisting_threads: BTreeSet<u32>,
        attach_query_error_count: u64,
        attach_query_errors: Vec<Value>,
        breakin_query_handles: BTreeMap<u32, Handle>,
        threads: BTreeMap<u32, Thread>,
        event_threads: BTreeMap<u32, Handle>,
        debug_process: Option<Handle>,
        exceptions: ExceptionPolicy,
        owns_probes: bool,
        forwarded_probe_context: bool,
        probe_resume_error: Option<String>,
        cleanup_exception_records: Vec<Value>,
        debug_trace: Vec<Value>,
        debug_trace_omitted: u64,
        debug_trace_omitted_by_kind: BTreeMap<String, u64>,
        module_event_seen: bool,
        ready: bool,
        pending: Option<Pending>,
        cleaned: bool,
        observation: Observation,
        installed_total: u32,
        detach_error: Option<String>,
    }

    impl Session {
        fn attach(process: Process, module: PinnedModule, ntdll: PinnedNtdll) -> Result<Self> {
            let preexisting_threads = preflight_threads(&process)?;
            module.verify(&process)?;
            let mut exceptions = ExceptionPolicy::default();
            exceptions.attach.bind(ntdll.verify(&process)?);
            process.verify()?;
            // SAFETY: only the PID whose creation/path/module were checked
            // against the still-owned process handle immediately above.
            if unsafe { DebugActiveProcess(process.pinned.pid) } == 0 {
                return Err(win_error("DebugActiveProcess"));
            }
            // No fallible operation between successful attach and its cleanup
            // owner. All subsequent errors pass through Session::finish/Drop.
            Ok(Self {
                process,
                module,
                ntdll,
                preexisting_threads,
                attach_query_error_count: 0,
                attach_query_errors: Vec::new(),
                breakin_query_handles: BTreeMap::new(),
                threads: BTreeMap::new(),
                event_threads: BTreeMap::new(),
                debug_process: None,
                exceptions,
                owns_probes: false,
                forwarded_probe_context: false,
                probe_resume_error: None,
                cleanup_exception_records: Vec::new(),
                debug_trace: Vec::new(),
                debug_trace_omitted: 0,
                debug_trace_omitted_by_kind: BTreeMap::new(),
                module_event_seen: false,
                ready: false,
                pending: None,
                cleaned: false,
                observation: Observation::default(),
                installed_total: 0,
                detach_error: None,
            })
        }

        fn finish(&mut self) -> CleanupReport {
            self.cleaned = true;
            let report = cleanup(self);
            self.trace(json!({"event":"exit","cleanup":report}));
            report
        }

        fn trace(&mut self, record: Value) {
            if retain_trace(
                &mut self.debug_trace,
                &mut self.debug_trace_omitted_by_kind,
                record,
            ) {
                self.debug_trace_omitted = self.debug_trace_omitted.saturating_add(1);
            }
        }

        fn resume_probe(
            &mut self,
            raw: RawException,
            decision: ExceptionDecision,
            value: CONTEXT,
            phase: &str,
        ) -> Result<()> {
            self.resume_probe_with_context_io(raw, decision, value, phase, set_context, context)
        }

        fn resume_probe_with_context_io(
            &mut self,
            raw: RawException,
            decision: ExceptionDecision,
            mut value: CONTEXT,
            phase: &str,
            set: fn(&Handle, CONTEXT) -> Result<()>,
            readback: fn(&Handle, u32) -> Result<CONTEXT>,
        ) -> Result<()> {
            let before = json!({"rip":value.Rip,"dr6":value.Dr6,
                "dr7":value.Dr7,"eflags":value.EFlags});
            value.ContextFlags = CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64;
            let (dr6, flags) = decision
                .resume_debug_flags(value.Dr6, value.EFlags)
                .ok_or("exception policy did not authorize probe resume")?;
            value.Dr6 = dr6;
            value.EFlags = flags;
            let handle = self
                .threads
                .get(&raw.tid)
                .map(|thread| &thread.handle)
                .or_else(|| self.event_threads.get(&raw.tid))
                .ok_or("owned probe has no thread handle")?;
            let updated = set(handle, value);
            // Read actual OS state, rather than reporting the requested RF as
            // proof it survived SetThreadContext. Do not repair RIP/game data.
            let actual_context = readback(handle, value.ContextFlags);
            let readback_check = validate_resume_readback(
                ProbeSnapshot {
                    armed: true,
                    registers: registers(&value),
                    rip: value.Rip,
                    flags: value.EFlags,
                },
                actual_context
                    .as_ref()
                    .map(|actual| ProbeSnapshot {
                        armed: true,
                        registers: registers(actual),
                        rip: actual.Rip,
                        flags: actual.EFlags,
                    })
                    .map_err(Clone::clone),
            );
            let actual = actual_context.map(|actual| {
                json!({"rip":actual.Rip,"dr6":actual.Dr6,"dr7":actual.Dr7,
                    "eflags":actual.EFlags})
            });
            self.trace(json!({"event":"probe_resume","phase":phase,"raw":raw,
                "decision":decision,"before":before,
                "requested":{"rip":value.Rip,"dr6":value.Dr6,"dr7":value.Dr7,"eflags":value.EFlags},
                "set_context_ok":updated.is_ok(),"set_context_error":updated.as_ref().err(),
                "readback_ok":readback_check.is_ok(),"readback_error":readback_check.as_ref().err(),
                "actual":raw_read(actual),"continue_status":self.pending.as_ref().map(|p|p.status)}));
            let result = updated.and(readback_check);
            if result.is_err() {
                if self.probe_resume_error.is_none() {
                    self.probe_resume_error = result.as_ref().err().cloned();
                }
                self.forwarded_probe_context |= !decision.handled();
                // No continuation after a failed resume until centralized
                // cleanup has attempted every owned register restoration.
                if let Some(pending) = &mut self.pending {
                    // Set/Get failure ends observation, not ownership of the
                    // fault already classified from a verified stopped context.
                    // Keep its original status; restore all probes before the
                    // ONE continuation. Never leak a known own fault to VEH.
                    pending.restore_before_continue = true;
                }
            }
            result
        }

        fn cleanup_exception(&mut self, event: &DEBUG_EVENT) -> Result<bool> {
            // SAFETY: only called for an owned EXCEPTION_DEBUG_EVENT.
            let exception = unsafe { event.u.Exception };
            let raw = RawException {
                tid: event.dwThreadId,
                code: exception.ExceptionRecord.ExceptionCode as u32,
                first_chance: exception.dwFirstChance,
                exception_address: exception.ExceptionRecord.ExceptionAddress as u64,
            };
            let snapshot = self
                .threads
                .get_mut(&raw.tid)
                .and_then(|thread| thread.cleanup_snapshot.take());
            let decision = self.route_exception(raw, snapshot)?;
            if matches!(
                decision,
                ExceptionDecision::ForwardFirstChance { record: true }
                    | ExceptionDecision::ForwardSecondChance
                    | ExceptionDecision::ForwardComposite(_)
                    | ExceptionDecision::RestoreAndForward
            ) {
                self.cleanup_exception_records.push(json!({"raw":raw,
                    "decision":decision,"continuation":"not_handled"}));
            }
            if decision.resume_debug_flags(0, 0).is_some() {
                let handle = self
                    .threads
                    .get(&raw.tid)
                    .map(|t| &t.handle)
                    .or_else(|| self.event_threads.get(&raw.tid))
                    .ok_or("owned cleanup probe has no thread handle")?;
                // Use CURRENT addresses/control, both before restoration and
                // while draining after it; never reinstall the captured probes.
                // Only the classified source is removed.
                let value = context(
                    handle,
                    CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64,
                )?;
                self.resume_probe(raw, decision, value, "cleanup")?;
            }
            Ok(decision.stop_observing())
        }

        fn register_os_start(
            &mut self,
            tid: u32,
            event_start: Option<u64>,
            start: Result<u64>,
        ) -> Result<bool> {
            if let Err(error) = &start {
                self.attach_query_error_count = self.attach_query_error_count.saturating_add(1);
                if self.attach_query_errors.len() < 16 {
                    self.attach_query_errors.push(json!({"tid":tid,
                    "raw_event_start":event_start,"query_error":error}));
                }
                return Ok(false);
            }
            self.exceptions.attach.register(
                tid,
                event_start,
                start,
                self.preexisting_threads.contains(&tid),
            )
        }

        fn register_cleanup_thread(&mut self, tid: u32, event_start: Option<u64>) -> Result<()> {
            if self.ready {
                return Ok(());
            }
            // SAFETY: event TID from the pinned PID; query only, owner verified
            // before using the handle. Keep a successful candidate's handle.
            let handle = Handle::new(
                unsafe { OpenThread(THREAD_QUERY_INFORMATION | THREAD_SYNCHRONIZE, 0, tid) },
                "OpenThread(cleanup attach identity)",
            )?;
            // SAFETY: query ownership on that same handle; never touch its context.
            if unsafe { GetProcessIdOfThread(handle.0) } != self.process.pinned.pid {
                return Err("cleanup attach thread owner mismatch".into());
            }
            self.process.verify()?;
            let start = self.ntdll.query.start_address(&handle);
            if self.register_os_start(tid, event_start, start)? {
                self.breakin_query_handles.insert(tid, handle);
            }
            Ok(())
        }

        fn add_thread(&mut self, tid: u32, raw: HANDLE, event_start: Option<u64>) -> Result<()> {
            let event_handle = Handle::new(raw, "debug event thread handle")?;
            if self.event_threads.contains_key(&tid) {
                return Err("duplicate live debug thread ID".into());
            }
            self.event_threads.insert(tid, event_handle);
            // Own a separate minimal-rights handle; Windows only auto-closes
            // the original event handle. It cannot close this cleanup handle.
            // SAFETY: explicit event TID, owner verified before any context set.
            let handle = Handle::new(
                unsafe {
                    OpenThread(
                        THREAD_GET_CONTEXT
                            | THREAD_SET_CONTEXT
                            | THREAD_SUSPEND_RESUME
                            | THREAD_QUERY_LIMITED_INFORMATION
                            | THREAD_QUERY_INFORMATION
                            | THREAD_SYNCHRONIZE,
                        0,
                        tid,
                    )
                },
                "OpenThread(debug event)",
            )?;
            // SAFETY: query the same owned thread handle before modifying it.
            if unsafe { GetProcessIdOfThread(handle.0) } != self.process.pinned.pid {
                return Err("debug-event thread owner mismatch".into());
            }
            self.threads.insert(
                tid,
                Thread {
                    handle,
                    original: DebugRegisters::default(),
                    armed: false,
                    enabled: 0x45,
                    suspended: false,
                    suspend_previous_count: None,
                    cleanup_snapshot: None,
                },
            );
            let thread = self.threads.get_mut(&tid).ok_or("missing new thread")?;
            let value = context(&thread.handle, CONTEXT_DEBUG_REGISTERS_AMD64)?;
            let original = registers(&value);
            thread.original = original;
            if !original.unused() {
                return Err(format!(
                    "pre-existing debug registers on attached/new thread {tid}"
                ));
            }
            if !self.ready {
                let start = self.ntdll.query.start_address(&thread.handle);
                self.register_os_start(tid, event_start, start)?;
            }
            if self.ready && !self.cleaned {
                self.install(tid)?;
            }
            Ok(())
        }

        fn install(&mut self, tid: u32) -> Result<()> {
            let thread = self
                .threads
                .get_mut(&tid)
                .ok_or("unknown thread to install")?;
            let mut value = context(&thread.handle, CONTEXT_DEBUG_REGISTERS_AMD64)?;
            if registers(&value) != thread.original || !thread.original.unused() {
                return Err("debug registers changed before installation".into());
            }
            let expected = thread
                .original
                .installed(self.module.addresses, thread.enabled);
            replace_registers(&mut value, expected);
            // Record intent BEFORE the API; even a failed/partial set must restore.
            thread.armed = true;
            self.owns_probes = true;
            set_context(&thread.handle, value)?;
            let installed = registers(&context(&thread.handle, CONTEXT_DEBUG_REGISTERS_AMD64)?);
            if installed.address != expected.address || installed.dr7 & !0x400 != thread.enabled {
                return Err("hardware probe installation readback mismatch".into());
            }
            self.installed_total += 1;
            self.trace(json!({"event":"probe_install","tid":tid,"actual":installed}));
            Ok(())
        }

        fn enable_trigger_getter(&mut self, tid: u32) -> Result<()> {
            // The debug event stops all threads. Only the retained setter
            // thread handle is changed; future threads still start at 0x45.
            let thread = self.threads.get_mut(&tid).ok_or("missing setter thread")?;
            let mut value = context(&thread.handle, CONTEXT_DEBUG_REGISTERS_AMD64)?;
            let before = registers(&value);
            let expected = thread
                .original
                .installed(self.module.addresses, thread.enabled);
            if !thread.armed
                || thread.enabled != 0x45
                || before.address != expected.address
                || before.dr7 & !0x400 != thread.enabled
            {
                return Err("setter thread probe readback changed before getter enable".into());
            }
            // Retain ownership intent before a potentially partial API write.
            thread.enabled = 0x55;
            value.Dr2 = self.module.addresses[2];
            value.Dr7 |= 0x10;
            set_context(&thread.handle, value)?;
            let actual = registers(&context(&thread.handle, CONTEXT_DEBUG_REGISTERS_AMD64)?);
            if actual.address != self.module.addresses || actual.dr7 & !0x400 != thread.enabled {
                return Err("trigger getter installation readback mismatch".into());
            }
            self.trace(
                json!({"event":"trigger_getter_enable","tid":tid,"before":before,"actual":actual}),
            );
            Ok(())
        }

        fn observe(
            &mut self,
            args: &Args,
            output: &mut File,
            cancelled: &AtomicBool,
        ) -> Result<&'static str> {
            // SAFETY: called on the attaching/debug-event thread, immediately
            // before any event is continued. Never allow debugger exit to kill Civ6.
            if unsafe { DebugSetProcessKillOnExit(0) } == 0 {
                return Err(win_error("DebugSetProcessKillOnExit(false)"));
            }
            let deadline = Instant::now() + Duration::from_secs(args.seconds);
            loop {
                if cancelled.load(Ordering::Relaxed) {
                    return Ok("cancelled");
                }
                let now = Instant::now();
                if now >= deadline {
                    return Ok("deadline");
                }
                let timeout = (deadline - now).as_millis().clamp(1, 100) as u32;
                let mut event = DEBUG_EVENT::default();
                // SAFETY: bounded wait on the same thread that attached.
                if unsafe { WaitForDebugEvent(&mut event, timeout) } == 0 {
                    // SAFETY: capture wait error before other API calls.
                    if unsafe { GetLastError() } == ERROR_SEM_TIMEOUT {
                        if !self.process.handle.alive()? {
                            return Ok("process_exited");
                        }
                        continue;
                    }
                    return Err(win_error("WaitForDebugEvent"));
                }
                // Own the event before any validation/I/O can fail. Exception
                // status starts NOT_HANDLED and only our positively identified
                // attach break/hardware probe can change it.
                self.pending = Some(Pending {
                    event,
                    restore_before_continue: false,
                    exception_routed: false,
                    decision: None,
                    status: if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT {
                        DBG_EXCEPTION_NOT_HANDLED
                    } else {
                        DBG_CONTINUE
                    },
                });
                let reason = self.event(args, output, &event)?;
                if let Some(reason) = reason {
                    // Keep this event stopped until every original DR is restored.
                    return Ok(reason);
                }
                self.continue_pending()?;
            }
        }

        fn event(
            &mut self,
            args: &Args,
            output: &mut File,
            event: &DEBUG_EVENT,
        ) -> Result<Option<&'static str>> {
            if event.dwProcessId != self.process.pinned.pid {
                return Err("debug event belongs to an unexpected process".into());
            }
            match event.dwDebugEventCode {
                CREATE_PROCESS_DEBUG_EVENT => {
                    // SAFETY: union arm selected by the debug event code.
                    let info = unsafe { event.u.CreateProcessInfo };
                    // Acquire all supplied handles before verification can fail.
                    let image_file = if info.hFile.is_null() {
                        None
                    } else {
                        Some(Handle::new(info.hFile, "debug image file")?)
                    };
                    self.debug_process = Some(Handle::new(info.hProcess, "debug process")?);
                    let thread_result = self.add_thread(
                        event.dwThreadId,
                        info.hThread,
                        info.lpStartAddress.map(|start| start as usize as u64),
                    );
                    let actual = identity(
                        self.debug_process
                            .as_ref()
                            .ok_or("missing debug process")?
                            .0,
                    )?;
                    match_identity(&self.process.pinned, &actual, self.process.handle.alive()?)?;
                    if info.lpStartAddress.is_some() {
                        return Err("event is not an existing-process attach".into());
                    }
                    if let Some(image_file) = image_file {
                        if path_key(&file_path(image_file.0)?)? != self.process.pinned.image_path {
                            return Err("debug image file path mismatch".into());
                        }
                    }
                    thread_result?;
                }
                CREATE_THREAD_DEBUG_EVENT => {
                    // SAFETY: arm selected by event code. All threads are stopped.
                    let info = unsafe { event.u.CreateThread };
                    self.add_thread(
                        event.dwThreadId,
                        info.hThread,
                        info.lpStartAddress.map(|start| start as usize as u64),
                    )?;
                }
                LOAD_DLL_DEBUG_EVENT => {
                    // SAFETY: arm selected by event code; close every hFile,
                    // including unrelated DLLs, without reading their memory.
                    let info = unsafe { event.u.LoadDll };
                    let file = if info.hFile.is_null() {
                        None
                    } else {
                        Some(Handle::new(info.hFile, "debug DLL file")?)
                    };
                    if info.lpBaseOfDll as u64 == self.module.identity.base {
                        let file = file.ok_or("pinned DLL debug event has no file handle")?;
                        if path_key(&file_path(file.0)?)? != self.module.identity.path {
                            return Err("DLL debug-event file path/base mismatch".into());
                        }
                        self.module_event_seen = true;
                    }
                }
                // SAFETY: the pattern selects the union arm before the guard.
                UNLOAD_DLL_DEBUG_EVENT
                    if unsafe { event.u.UnloadDll.lpBaseOfDll } as u64
                        == self.module.identity.base =>
                {
                    return Err("pinned DLL unloaded".into());
                }
                EXIT_THREAD_DEBUG_EVENT => {
                    // Original event handle is retired only after Continue succeeds.
                }
                EXIT_PROCESS_DEBUG_EVENT => {
                    // SAFETY: arm selected by event code, no memory read.
                    write_json(
                        output,
                        &json!({"event":"target_exit", "raw_exit_code": unsafe { event.u.ExitProcess.dwExitCode }}),
                    )?;
                    return Ok(Some("process_exited"));
                }
                EXCEPTION_DEBUG_EVENT => return self.exception(args, output, event),
                RIP_EVENT => return Err("Windows reported a debugger RIP event".into()),
                _ => {} // no remote reads of debug strings, names, or whole objects
            }
            Ok(None)
        }

        fn apply_decision(&mut self, decision: ExceptionDecision) {
            if let Some(pending) = &mut self.pending {
                pending.restore_before_continue = decision.stop_observing();
                pending.exception_routed = true;
                pending.decision = Some(decision);
                pending.status = if decision.handled() {
                    DBG_CONTINUE
                } else {
                    DBG_EXCEPTION_NOT_HANDLED
                };
            }
        }

        fn route_exception(
            &mut self,
            raw: RawException,
            snapshot: Option<ProbeSnapshot>,
        ) -> Result<ExceptionDecision> {
            if self.exceptions.attach.matches(raw) {
                // Recheck the pinned image and the same event-thread handle at
                // the actual break, rather than relying on a stale registration.
                self.ntdll.verify(&self.process)?;
                let handle = self
                    .threads
                    .get(&raw.tid)
                    .map(|thread| &thread.handle)
                    .or_else(|| self.breakin_query_handles.get(&raw.tid))
                    .ok_or("registered attach thread handle missing")?;
                let start = self.ntdll.query.start_address(handle)?;
                if self.exceptions.attach.breakin_threads.get(&raw.tid) != Some(&start) {
                    self.exceptions.attach.retire(raw.tid);
                    return Err("registered break-in thread OS start changed".into());
                }
            }
            let enabled = self
                .threads
                .get(&raw.tid)
                .filter(|_| self.owns_probes)
                .map_or(0, |thread| thread.enabled);
            let decision = self
                .exceptions
                .decide(raw, snapshot, self.module.addresses, enabled);
            if matches!(
                decision,
                ExceptionDecision::ForwardComposite(_) | ExceptionDecision::RestoreAndForward
            ) {
                self.forwarded_probe_context = true;
            }
            self.apply_decision(decision);
            Ok(decision)
        }

        fn exception(
            &mut self,
            args: &Args,
            output: &mut File,
            event: &DEBUG_EVENT,
        ) -> Result<Option<&'static str>> {
            // SAFETY: caller selected EXCEPTION_DEBUG_EVENT.
            let exception = unsafe { event.u.Exception };
            let code = exception.ExceptionRecord.ExceptionCode;
            let raw = RawException {
                tid: event.dwThreadId,
                code: code as u32,
                first_chance: exception.dwFirstChance,
                exception_address: exception.ExceptionRecord.ExceptionAddress as u64,
            };
            let thread = self.threads.get(&event.dwThreadId);
            let context_result = if code == EXCEPTION_SINGLE_STEP && self.owns_probes {
                thread.map(|thread| {
                    context(
                        &thread.handle,
                        CONTEXT_DEBUG_REGISTERS_AMD64
                            | CONTEXT_CONTROL_AMD64
                            | CONTEXT_INTEGER_AMD64,
                    )
                })
            } else {
                None
            };
            let context_error = context_result
                .as_ref()
                .and_then(|result| result.as_ref().err());
            let snapshot = context_result
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                .map(|value| ProbeSnapshot {
                    armed: thread.is_some_and(|thread| thread.armed),
                    registers: registers(value),
                    rip: value.Rip,
                    flags: value.EFlags,
                });
            let decision = self.route_exception(raw, snapshot)?;
            // Set continuation BEFORE logging, raw reads, or context updates.
            // The same policy is used when draining queued events in cleanup.
            if decision == ExceptionDecision::AttachBreak {
                // Attach's synthetic break is the only software exception we handle.
                if !self.module_event_seen {
                    return Err("attach break arrived without the pinned DLL event".into());
                }
                self.process.verify()?;
                self.module.verify(&self.process)?;
                let tids: Vec<_> = self.threads.keys().copied().collect();
                if tids.is_empty() {
                    return Err("no attach threads were reported".into());
                }
                for tid in tids {
                    self.install(tid)?;
                }
                self.ready = true;
                let ready = json!({"event":"ready", "pid":args.pid, "created":args.created,
                    "image_path":self.process.pinned.image_path, "module":self.module.identity,
                    "module_file_sha256":MODULE_SHA256, "probe_rvas":RVAS,
                    "threads_installed":self.installed_total, "ready_path":args.ready_path(),
                    "initial_local_enable_mask":0x45,"getter_enable_policy":"first_setter_thread_only",
                    "attach_break":self.exceptions.attach, "ntdll_identity":self.ntdll.report(),
                    "raw_notification_interface":self.process.interface_fields(self.module.identity.base),
                    "raw_main_image_module":self.process.main_image_fields(),
                    "semantics":"raw_fields_only"});
                let mut marker = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(args.ready_path())
                    .map_err(|e| format!("create_new ready file: {e}"))?;
                write_json(&mut marker, &ready)?;
                write_json(output, &ready)?;
                write_json(&mut std::io::stdout().lock(), &ready)?;
                return Ok(None);
            }
            if let ExceptionDecision::ForwardFirstChance { record } = decision {
                if record {
                    write_json(
                        output,
                        &json!({"event":"forwarded_first_chance", "raw":raw,
                    "context_error":context_error, "continuation":"not_handled"}),
                    )?;
                }
                return Ok(None);
            }
            if decision == ExceptionDecision::ForwardSecondChance {
                write_json(
                    output,
                    &json!({"event":"forwarded_second_chance", "raw":raw,
                    "continuation":"not_handled"}),
                )?;
                return Ok(Some("second_chance_exception"));
            }
            if decision == ExceptionDecision::RestoreAndForward {
                let record = json!({"event":"owned_probe_identity_uncertain", "raw":raw,
                    "raw_context":snapshot, "context_error":context_error,
                    "continuation":"not_handled_after_restore"});
                self.trace(record.clone());
                write_json(output, &record)?;
                return Ok(Some("owned_probe_identity_uncertain"));
            }
            let value = match context_result.and_then(std::result::Result::ok) {
                Some(value) => value,
                None => return Err("owned probe context unavailable".into()),
            };
            self.resume_probe(raw, decision, value, "observe")?;
            if let ExceptionDecision::ProbeAndStopForAppTF(source) = decision {
                write_json(
                    output,
                    &json!({"event":"owned_fault_before_app_tf",
                    "raw":raw,"source":source,"continuation":"handled_after_restore"}),
                )?;
                return Ok(Some("app_tf_pending_after_owned_fault"));
            }
            if let ExceptionDecision::ForwardComposite(source) = decision {
                write_json(
                    output,
                    &json!({"event":"forwarded_composite_single_step",
                    "raw":raw,"source":source,"continuation":"not_handled_after_restore"}),
                )?;
                return Ok(Some("composite_single_step"));
            }
            let ExceptionDecision::Probe(source) = decision else {
                return Err("unexpected observation decision".into());
            };
            let fields = match source.slot {
                0 if self.observation.setter(
                    value.Rbx,
                    value.Rcx,
                    event.dwThreadId,
                    value.Rsp,
                )? =>
                {
                    let trigger = self.observation.trigger.ok_or("missing setter trigger")?;
                    self.enable_trigger_getter(event.dwThreadId)?;
                    Some(json!({"event":"setter_before", "trigger":trigger,
                        "raw_rdx_pointer":value.Rdx, "raw_delta_i32":raw_read(self.process.i32_at(value.Rdx, 0)),
                        "fields":self.process.cache_fields(trigger)}))
                }
                1 if self
                    .observation
                    .returned(value.Rbx, event.dwThreadId, value.Rsp) =>
                {
                    Some(
                        json!({"event":"setter_return", "trigger":self.observation.trigger,
                        "fields":self.process.cache_fields(self.observation.trigger.ok_or("missing trigger")?)}),
                    )
                }
                2 if self.observation.getter() => Some(
                    json!({"event":"getter_raw", "ordinal":self.observation.getters_recorded,
                        "raw_rax_pointer":value.Rax, "raw_rax_plus_8_i32":raw_read(self.process.i32_at(value.Rax, 8)),
                        "correlation":"first_setter_thread_after_return_only", "city_identity_verified":false}),
                ),
                3 if self.observation.candidate(value.Rbx, event.dwThreadId) => {
                    let record = self.process.read::<8>(value.Rdx);
                    Some(
                        json!({"event":"candidate_call_raw", "ordinal":self.observation.candidates_recorded,
                        "raw_rbx_receiver":value.Rbx, "raw_rdx_pointer":value.Rdx, "raw_rcx_value":value.Rcx,
                        "rcx_interpretation":"unverified_value_not_a_header_pointer",
                        "raw_record_bytes":raw_read(record.clone()),
                        "raw_record_0_u16":raw_read(record.clone().map(|r| u16::from_le_bytes([r[0], r[1]]))),
                        "raw_record_2_u16":raw_read(record.clone().map(|r| u16::from_le_bytes([r[2], r[3]]))),
                        "raw_record_4_u32":raw_read(record.map(|r| u32::from_le_bytes([r[4], r[5], r[6], r[7]]))),
                        "raw_receiver_plus_90_pointer":raw_read(self.process.u64_at(value.Rbx, 0x90))}),
                    )
                }
                _ => None,
            };
            if let Some(mut fields) = fields {
                fields["tid"] = json!(event.dwThreadId);
                fields["raw_rip"] = json!(value.Rip);
                fields["probe_rva"] = json!(RVAS[source.slot]);
                write_json(output, &fields)?;
            }
            if self.observation.another_setter_for_trigger_cache {
                return Ok(Some("another_setter_for_trigger_cache"));
            }
            if self.observation.getters_recorded >= GETTER_LIMIT {
                return Ok(Some("getter_limit"));
            }
            if self.observation.candidates_recorded >= CANDIDATE_LIMIT {
                return Ok(Some("candidate_limit"));
            }
            Ok(None)
        }
    }

    impl CleanupBackend for Session {
        fn freeze(&mut self) -> Vec<String> {
            let mut errors = Vec::new();
            // Exit events cannot execute another instruction. Release Windows'
            // event stop before checking the held process/thread wait states;
            // do not attempt SuspendThread on a terminating target.
            if self.pending.as_ref().is_some_and(|pending| {
                matches!(pending.event.dwDebugEventCode, EXIT_PROCESS_DEBUG_EVENT)
            }) {
                if let Err(error) = self.continue_pending() {
                    errors.push(error);
                }
            }
            for (&tid, thread) in &mut self.threads {
                if !thread.armed {
                    continue;
                }
                match thread.handle.alive() {
                    Ok(false) => continue,
                    Err(error) => {
                        errors.push(format!("freeze thread {tid}: {error}"));
                        continue;
                    }
                    Ok(true) => {}
                }
                // SAFETY: same owned target thread handle. Retain exactly one
                // suspend count across restore, event drainage, and detach.
                let previous_count = unsafe { SuspendThread(thread.handle.0) };
                if previous_count == u32::MAX {
                    errors.push(format!(
                        "freeze thread {tid}: {}",
                        win_error("SuspendThread")
                    ));
                    continue;
                }
                thread.suspended = true;
                thread.suspend_previous_count = Some(previous_count);
                match context(
                    &thread.handle,
                    CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64,
                ) {
                    Ok(value) => {
                        // Preserve the pre-restore evidence. The shared policy
                        // will select a slot even when B0..B3 have accumulated.
                        thread.cleanup_snapshot = Some(ProbeSnapshot {
                            armed: thread.armed,
                            registers: registers(&value),
                            rip: value.Rip,
                            flags: value.EFlags,
                        });
                    }
                    Err(error) => {
                        errors.push(format!("capture queued probe on thread {tid}: {error}"))
                    }
                }
            }
            // Classify the outstanding event before restoring any DRs. The
            // same routine handles queued exceptions in drain_events.
            if self.pending.as_ref().is_some_and(|p| {
                p.event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT && !p.exception_routed
            }) {
                let event = self.pending.as_ref().expect("checked pending event").event;
                if let Err(error) = self.cleanup_exception(&event) {
                    errors.push(error);
                }
            }
            errors
        }

        fn armed_threads(&self) -> Vec<u32> {
            self.threads
                .iter()
                .filter_map(|(&tid, thread)| thread.armed.then_some(tid))
                .collect()
        }

        fn restore(&mut self, tid: u32) -> Result<RestoreOutcome> {
            let known_owned_bits = self
                .pending
                .as_ref()
                .filter(|p| p.event.dwThreadId == tid)
                .and_then(|p| p.decision)
                .and_then(|d| match d {
                    ExceptionDecision::Probe(source)
                    | ExceptionDecision::ProbeAndStopForAppTF(source) => Some(source.owned_b_bits),
                    _ => None,
                })
                .unwrap_or(0);
            let thread = self.threads.get_mut(&tid).ok_or("missing cleanup thread")?;
            if !thread.handle.alive()? {
                thread.armed = false;
                return Ok(RestoreOutcome::ThreadExited);
            }
            if !thread.suspended {
                // SAFETY: retry the reversible freeze on this same target
                // handle; a failed freeze must not lead to setting a running context.
                let previous_count = unsafe { SuspendThread(thread.handle.0) };
                if previous_count == u32::MAX {
                    let error = win_error("SuspendThread(restore)");
                    if !thread.handle.alive()? {
                        return Ok(RestoreOutcome::ThreadExited);
                    }
                    return Err(error);
                }
                thread.suspended = true;
                thread.suspend_previous_count = Some(previous_count);
            }
            let mut value = context(
                &thread.handle,
                CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64,
            )?;
            // Restore the addresses/control we own, retaining the application's
            // status. Confirmed probe bits were removed by the common resume
            // policy; uncertain/second-chance bits must not be guessed away.
            let expected = DebugRegisters {
                dr6: (thread.original.dr6 & !0xE00F) | (value.Dr6 & 0xE00F & !known_owned_bits),
                ..thread.original
            };
            replace_registers(&mut value, expected);
            let requested = ProbeSnapshot {
                armed: false,
                registers: expected,
                rip: value.Rip,
                flags: value.EFlags,
            };
            value.ContextFlags = CONTEXT_DEBUG_REGISTERS_AMD64;
            let updated = set_context(&thread.handle, value);
            let actual_context = context(
                &thread.handle,
                CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64,
            );
            let verified = validate_resume_readback(
                requested,
                actual_context
                    .as_ref()
                    .map(|v| ProbeSnapshot {
                        armed: false,
                        registers: registers(v),
                        rip: v.Rip,
                        flags: v.EFlags,
                    })
                    .map_err(Clone::clone),
            );
            let actual = actual_context.map(|v| {
                json!({"address":registers(&v).address,
                "dr6":v.Dr6,"dr7":v.Dr7,"rip":v.Rip,"eflags":v.EFlags})
            });
            let record = json!({"event":"probe_restore","tid":tid,"requested":expected,
                "actual":raw_read(actual),"original":thread.original,
                "control_before":{"rip":requested.rip,"eflags":requested.flags},
                "known_owned_bits_removed":known_owned_bits,
                "set_context_ok":updated.is_ok(),"readback_ok":verified.is_ok(),"readback_error":verified.as_ref().err()});
            if updated.is_ok() && verified.is_ok() {
                thread.armed = false;
            }
            self.trace(record);
            updated?;
            verified.map_err(|e| format!("original debug-register readback mismatch: {e}"))?;
            Ok(RestoreOutcome::Restored)
        }

        fn continue_pending(&mut self) -> Result<()> {
            let classification_error = if self.cleaned
                && self.pending.as_ref().is_some_and(|p| {
                    p.event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT && !p.exception_routed
                }) {
                let event = self.pending.as_ref().ok_or("missing cleanup event")?.event;
                self.cleanup_exception(&event).err()
            } else {
                None
            };
            if let Some(error) = &classification_error {
                self.trace(json!({"event":"error","phase":"cleanup_exception","error":error}));
            }
            let Some(pending) = &self.pending else {
                return Ok(());
            };
            let event = pending.event;
            if pending.restore_before_continue && !self.cleaned {
                return Err(
                    "terminal exception requires centralized probe restoration before continue"
                        .into(),
                );
            }
            // SAFETY: exactly the event returned to this attaching thread;
            // status was defaulted conservatively before any fallible handler.
            let status = if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT
                && unsafe { event.u.Exception.dwFirstChance } == 0
            {
                DBG_EXCEPTION_NOT_HANDLED
            } else {
                pending.status
            };
            let ok =
                unsafe { ContinueDebugEvent(event.dwProcessId, event.dwThreadId, status) } != 0;
            let error = (!ok).then(|| win_error("ContinueDebugEvent"));
            // DLL-load chatter must not exhaust the finite trap trace before
            // the first real probe. Keep exceptions, exits, and API failures.
            if !ok
                || matches!(
                    event.dwDebugEventCode,
                    EXCEPTION_DEBUG_EVENT | EXIT_PROCESS_DEBUG_EVENT
                )
            {
                let raw = (event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT).then(|| {
                    // SAFETY: the event code selects this union arm.
                    let info = unsafe { event.u.Exception };
                    RawException {
                        tid: event.dwThreadId,
                        code: info.ExceptionRecord.ExceptionCode as u32,
                        first_chance: info.dwFirstChance,
                        exception_address: info.ExceptionRecord.ExceptionAddress as u64,
                    }
                });
                self.trace(json!({"event":"debug_continue","pid":event.dwProcessId,
                    "tid":event.dwThreadId,"debug_event_code":event.dwDebugEventCode,"raw":raw,
                    "continue_status":status,"ok":ok,"error":error}));
            }
            if let Some(error) = error {
                return Err(error);
            }
            self.pending = None;
            if matches!(
                event.dwDebugEventCode,
                EXIT_THREAD_DEBUG_EVENT | EXIT_PROCESS_DEBUG_EVENT
            ) {
                if let Some(handle) = self.event_threads.remove(&event.dwThreadId) {
                    handle.closed_by_continue();
                }
                self.threads.remove(&event.dwThreadId);
                self.exceptions.attach.retire(event.dwThreadId);
                self.breakin_query_handles.remove(&event.dwThreadId);
            }
            if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
                if let Some(process) = self.debug_process.take() {
                    process.closed_by_continue();
                }
                // EXIT_PROCESS is delivered before the held process handle is
                // necessarily signalled. An immediate zero-time check can
                // falsely report a dead target as successfully restored alive.
                // SAFETY: same independent process handle retained since open.
                let wait = unsafe { WaitForSingleObject(self.process.handle.0, 3000) };
                self.trace(json!({"event":"target_exit_wait","wait_result":wait,
                    "process_exited":wait == WAIT_OBJECT_0}));
                if wait != WAIT_OBJECT_0 {
                    return Err(if wait == WAIT_TIMEOUT {
                        "target exit was not signalled within 3 seconds".into()
                    } else {
                        win_error("WaitForSingleObject(target exit)")
                    });
                }
            }
            classification_error.map_or(Ok(()), Err)
        }

        fn drain_events(&mut self) -> Result<()> {
            if self.pending.is_some() {
                return Err("cannot drain events after a failed ContinueDebugEvent".into());
            }
            if !self.process.handle.alive()? {
                return Ok(());
            }
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut errors = Vec::new();
            // All threads with our probes retain their own suspend count, so
            // they cannot generate more hits. A previously queued hit must be
            // continued after its DRs were restored, before detaching.
            for _ in 0..8192 {
                let mut event = DEBUG_EVENT::default();
                // SAFETY: nonblocking drain on the same attaching thread.
                if unsafe { WaitForDebugEvent(&mut event, 0) } == 0 {
                    // SAFETY: capture this API's error immediately.
                    if unsafe { GetLastError() } != ERROR_SEM_TIMEOUT {
                        errors.push(win_error("WaitForDebugEvent(cleanup)"));
                    }
                    return if errors.is_empty() {
                        Ok(())
                    } else {
                        Err(errors.join("; "))
                    };
                }
                self.pending = Some(Pending {
                    event,
                    restore_before_continue: false,
                    exception_routed: false,
                    decision: None,
                    status: if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT {
                        DBG_EXCEPTION_NOT_HANDLED
                    } else {
                        DBG_CONTINUE
                    },
                });
                let mut terminal = false;
                let handled = (|| -> Result<()> {
                    if event.dwProcessId != self.process.pinned.pid {
                        return Err("unexpected process in cleanup event".into());
                    }
                    match event.dwDebugEventCode {
                        CREATE_PROCESS_DEBUG_EVENT => {
                            // SAFETY: event code selects this union arm.
                            let info = unsafe { event.u.CreateProcessInfo };
                            let _file = if info.hFile.is_null() {
                                None
                            } else {
                                Some(Handle::new(info.hFile, "cleanup image file")?)
                            };
                            self.debug_process =
                                Some(Handle::new(info.hProcess, "cleanup process handle")?);
                            self.event_threads.insert(
                                event.dwThreadId,
                                Handle::new(info.hThread, "cleanup initial thread")?,
                            );
                            self.register_cleanup_thread(
                                event.dwThreadId,
                                info.lpStartAddress.map(|start| start as usize as u64),
                            )?;
                        }
                        CREATE_THREAD_DEBUG_EVENT => {
                            // SAFETY: event code selects this union arm. Never
                            // install probes while cleaning up a queued new thread.
                            let info = unsafe { event.u.CreateThread };
                            self.event_threads.insert(
                                event.dwThreadId,
                                Handle::new(info.hThread, "cleanup new thread")?,
                            );
                            self.register_cleanup_thread(
                                event.dwThreadId,
                                info.lpStartAddress.map(|start| start as usize as u64),
                            )?;
                        }
                        LOAD_DLL_DEBUG_EVENT => {
                            // SAFETY: file handle from this union arm is ours.
                            let file = unsafe { event.u.LoadDll.hFile };
                            if !file.is_null() {
                                let _file = Handle::new(file, "cleanup DLL file")?;
                            }
                        }
                        EXCEPTION_DEBUG_EVENT => {
                            terminal = self.cleanup_exception(&event)?;
                        }
                        _ => {}
                    }
                    Ok(())
                })();
                if let Err(error) = handled {
                    errors.push(error);
                }
                // Continue is attempted even if capture/validation failed.
                if let Err(error) = self.continue_pending() {
                    errors.push(error);
                    return Err(errors.join("; "));
                }
                if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
                    return if errors.is_empty() {
                        Ok(())
                    } else {
                        Err(errors.join("; "))
                    };
                }
                if terminal {
                    return if errors.is_empty() {
                        Ok(())
                    } else {
                        Err(errors.join("; "))
                    };
                }
                // A genuine second-chance target exception ends observation;
                // it was continued NOT_HANDLED, never converted into read_error.
                if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT {
                    // SAFETY: selected union arm.
                    if unsafe { event.u.Exception.dwFirstChance } == 0 {
                        return if errors.is_empty() {
                            Ok(())
                        } else {
                            Err(errors.join("; "))
                        };
                    }
                }
                if Instant::now() >= deadline {
                    break;
                }
            }
            errors.push("bounded cleanup event-drain limit reached".into());
            Err(errors.join("; "))
        }

        fn detach(&mut self) -> Result<bool> {
            // SAFETY: same PID/debugger thread; no termination or memory write.
            if unsafe { DebugActiveProcessStop(self.process.pinned.pid) } != 0 {
                return Ok(true);
            }
            let error = win_error("DebugActiveProcessStop");
            self.detach_error = Some(error.clone());
            if !self.process.handle.alive()? {
                // Still report the API's actual failure, without calling an
                // already exited target a successful detach.
                return Ok(false);
            }
            Err(error)
        }

        fn detach_diagnostic(&self) -> Option<String> {
            self.detach_error.clone()
        }

        fn resume(&mut self) -> Vec<String> {
            let mut errors = Vec::new();
            let mut records = Vec::new();
            for (&tid, thread) in &mut self.threads {
                if !thread.suspended {
                    continue;
                }
                // SAFETY: remove exactly the suspend count owned by cleanup,
                // after detach has released the debugger's separate event stop.
                let previous_count = unsafe { ResumeThread(thread.handle.0) };
                let expected_count = thread.suspend_previous_count.and_then(|n| n.checked_add(1));
                let released_expected_count = Some(previous_count) == expected_count;
                records.push(json!({"event":"thread_resume","tid":tid,"previous_count":previous_count,
                    "expected_count":expected_count,"ok":released_expected_count && previous_count != u32::MAX,
                    "runnable":previous_count == 1}));
                if previous_count == u32::MAX {
                    let error = win_error("ResumeThread");
                    if thread.handle.alive() != Ok(false) {
                        errors.push(format!("resume thread {tid}: {error}"));
                    }
                } else {
                    thread.suspended = false;
                    if !released_expected_count {
                        errors.push(format!("resume thread {tid}: suspend count {previous_count}, expected {expected_count:?}"));
                    }
                    // Counts above one preserve an application's preexisting
                    // suspension. Releasing our one count does not imply that
                    // such a thread became runnable; never resume it again.
                }
            }
            for record in records {
                self.trace(record);
            }
            errors
        }

        fn process_exited(&self) -> bool {
            self.process.handle.alive() == Ok(false)
        }
    }

    impl Drop for Session {
        fn drop(&mut self) {
            if !self.cleaned {
                let report = self.finish();
                // Fallback after an unexpected unwind, independent of file I/O.
                eprintln!("debug session cleanup: {report:?}");
            }
        }
    }

    fn write_json(writer: &mut impl Write, value: &Value) -> Result<()> {
        serde_json::to_writer(&mut *writer, value).map_err(|e| format!("write JSON: {e}"))?;
        writer
            .write_all(b"\n")
            .map_err(|e| format!("write JSON newline: {e}"))?;
        writer.flush().map_err(|e| format!("flush JSON: {e}"))
    }

    pub fn run(args: &Args) -> Result<()> {
        let setup = (|| -> Result<File> {
            args.validate()?;
            match std::fs::symlink_metadata(args.ready_path()) {
                Ok(_) => return Err("ready file already exists; use a new output name".into()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("check new ready path: {error}")),
            }
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&args.output)
                .map_err(|e| format!("create_new output: {e}"))
        })();
        let mut output = match setup {
            Ok(file) => file,
            Err(error) => {
                write_json(
                    &mut std::io::stdout().lock(),
                    &json!({"event":"exit", "reason":"error",
                    "error":error, "ready_reached":false, "cleanup_complete":true,
                    "attach_attempted":false, "semantics":"raw_fields_only", "product_acceptance":false}),
                )?;
                return Err(error);
            }
        };
        let started = Instant::now();
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&cancelled);
        let mut session = None;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            ctrlc::set_handler(move || {
                signal.store(true, Ordering::Relaxed);
            })
            .map_err(|e| format!("install cancellation handler: {e}"))?;
            let process = Process::open(args)?;
            let module = PinnedModule::open(&process)?;
            let ntdll = PinnedNtdll::open(&process)?;
            session = Some(Session::attach(process, module, ntdll)?);
            session
                .as_mut()
                .ok_or("missing debug session")?
                .observe(args, &mut output, &cancelled)
        }))
        .unwrap_or_else(|_| Err("observer panicked; running centralized cleanup".into()));
        let outcome = require_ready(
            outcome,
            session.as_ref().is_some_and(|session| session.ready),
        );
        // The only orderly cleanup path, before any completion output can fail.
        let cleanup_report = session.as_mut().map(Session::finish);
        let api_complete = cleanup_report.as_ref().is_none_or(CleanupReport::complete);
        // NOT_HANDLED can deliver the original exception CONTEXT despite a
        // successful Set/GetThreadContext readback, including installed DR7.
        // The owned-child acceptance tests keep this OS discrepancy visible.
        let delivery_unverified = session.as_ref().is_some_and(|s| s.forwarded_probe_context)
            && !cleanup_report.as_ref().is_some_and(|r| r.process_exited);
        let complete = api_complete && !delivery_unverified;
        let exit = json!({"event":"exit", "pid":args.pid, "created":args.created,
            "reason":outcome.as_ref().copied().unwrap_or("error"), "error":outcome.as_ref().err(),
            "ready_reached":session.as_ref().is_some_and(|s| s.ready),
            "elapsed_ms":started.elapsed().as_millis(), "cleanup_complete":complete,
            "debugger_api_cleanup_complete":api_complete,
            "forwarded_probe_context_delivery_unverified":delivery_unverified,
            "probe_resume_error":session.as_ref().and_then(|s| s.probe_resume_error.as_ref()),
            "cleanup":cleanup_report, "observation":session.as_ref().map(|s| &s.observation),
            "threads_installed_total":session.as_ref().map_or(0, |s| s.installed_total),
            "exceptions":session.as_ref().map(|s| &s.exceptions),
            "cleanup_exception_records":session.as_ref().map(|s| &s.cleanup_exception_records),
            "debug_trace":session.as_ref().map(|s| &s.debug_trace),
            "debug_trace_omitted":session.as_ref().map_or(0, |s| s.debug_trace_omitted),
            "debug_trace_omitted_by_kind":session.as_ref().map(|s| &s.debug_trace_omitted_by_kind),
            "held_process_alive":session.as_ref().map(|s| raw_read(s.process.handle.alive())),
            "live_target_restoration_complete":complete && cleanup_report.as_ref().is_some_and(|r| !r.process_exited),
            "ntdll_identity":session.as_ref().map(|s| s.ntdll.report()),
            "attach_query_error_count":session.as_ref().map_or(0, |s| s.attach_query_error_count),
            "attach_query_errors":session.as_ref().map(|s| &s.attach_query_errors),
            "semantics":"raw_fields_only", "product_acceptance":false});
        let file_result = write_json(&mut output, &exit);
        let stdout_result = write_json(&mut std::io::stdout().lock(), &exit);
        file_result?;
        stdout_result?;
        outcome?;
        if !complete {
            return Err("debug session cleanup incomplete; inspect exit report".into());
        }
        Ok(())
    }

    #[cfg(test)]
    mod live_tests {
        use super::*;
        use std::os::windows::process::CommandExt;
        use std::process::{Child, Command, Stdio};
        use windows_sys::Win32::Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS};
        use windows_sys::Win32::System::Diagnostics::Debug::{
            AddVectoredExceptionHandler, RaiseException, RemoveVectoredExceptionHandler,
            EXCEPTION_POINTERS,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, CREATE_NO_WINDOW};

        static APP_STEPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        static APP_DR6: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        static APP_FLAGS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        static APP_RIP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        static APP_DR7: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        static APP_CODE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        static APP_ADDRESS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        static APP_TF_HANDLED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        static APP_EVENTS: std::sync::Mutex<Vec<Value>> = std::sync::Mutex::new(Vec::new());
        static APP_UNHANDLED: AtomicBool = AtomicBool::new(false);
        static APP_EXPECT_NEXT_TF: AtomicBool = AtomicBool::new(false);
        fn app_report(normal_result: Option<u64>) -> Value {
            json!({"app_steps":APP_STEPS.load(Ordering::Relaxed),
                "first_code":APP_CODE.load(Ordering::Relaxed),"first_address":APP_ADDRESS.load(Ordering::Relaxed),
                "first_dr6":APP_DR6.load(Ordering::Relaxed),"first_eflags":APP_FLAGS.load(Ordering::Relaxed),
                "first_dr7":APP_DR7.load(Ordering::Relaxed),"first_rip":APP_RIP.load(Ordering::Relaxed),
                "app_tf_handled":APP_TF_HANDLED.load(Ordering::Relaxed),"normal_result":normal_result,
                "events":*APP_EVENTS.lock().unwrap()})
        }
        unsafe extern "system" fn app_exception(info: *mut EXCEPTION_POINTERS) -> i32 {
            // SAFETY: Windows supplies these exception structures during VEH.
            if unsafe { (*(*info).ExceptionRecord).ExceptionCode } == EXCEPTION_SINGLE_STEP {
                let value = unsafe { &mut *(*info).ContextRecord };
                let exception = unsafe { &*(*info).ExceptionRecord };
                if APP_STEPS.fetch_add(1, Ordering::Relaxed) == 0 {
                    APP_DR6.store(value.Dr6, Ordering::Relaxed);
                    APP_FLAGS.store(value.EFlags, Ordering::Relaxed);
                    APP_RIP.store(value.Rip, Ordering::Relaxed);
                    APP_DR7.store(value.Dr7, Ordering::Relaxed);
                    APP_CODE.store(exception.ExceptionCode as u32, Ordering::Relaxed);
                    APP_ADDRESS.store(exception.ExceptionAddress as u64, Ordering::Relaxed);
                }
                let before_flags = value.EFlags;
                let leaked_owned_fault = value.Dr6 & 0xF != 0;
                // With DR7 restored to zero Windows delivers DR6=0, including
                // BS, in VEH on this OS. Verify the actual after-instruction
                // boundary of the child's fixed four-byte LEA instead. No
                // synthetic DR6 bits are returned or added to the report.
                let next_tf_boundary = APP_EXPECT_NEXT_TF.load(Ordering::Relaxed)
                    && value.Rip == native_getter as *const () as u64 + 4
                    && exception.ExceptionAddress as u64 == value.Rip
                    && value.Dr7 & !0x400 == 0
                    && value.Dr6 & 0xE00F == 0;
                let handles_tf =
                    !leaked_owned_fault && (value.Dr6 & 0x4000 != 0 || next_tf_boundary);
                if handles_tf {
                    // Only the application's GENUINE BS trap is handled here.
                    // Never clear B bits/DR7 or set RF to hide an owned leak.
                    value.EFlags &= !0x100;
                    APP_TF_HANDLED.fetch_add(1, Ordering::Relaxed);
                }
                APP_EVENTS.lock().unwrap().push(json!({"code":exception.ExceptionCode as u32,
                    "address":exception.ExceptionAddress as u64,"rip":value.Rip,
                    "dr6":value.Dr6,"dr7":value.Dr7,"eflags_before":before_flags,
                    "eflags_after":value.EFlags,"handled_tf":handles_tf,"next_tf_instruction_boundary":next_tf_boundary,
                    "leaked_owned_fault":leaked_owned_fault}));
                // Record even the rejected leak before Windows terminates the
                // child, so an exit is never mistaken for a safe continuation.
                if let Some(path) = std::env::var_os("LOYALTY_OWNED_CHILD_REPORT") {
                    if let Ok(mut file) = OpenOptions::new().write(true).truncate(true).open(path) {
                        let _ = write_json(&mut file, &app_report(None));
                    }
                }
                if leaked_owned_fault || APP_UNHANDLED.load(Ordering::Relaxed) {
                    return 0;
                }
                -1 // EXCEPTION_CONTINUE_EXECUTION: application handled its own exception
            } else {
                0
            }
        }

        // Distinct bodies prevent linker folding. Real execution breakpoints,
        // no int3 patches, injected DLLs, remote calls, or game processes.
        #[inline(never)]
        extern "C" fn setter(n: u64) -> u64 {
            std::hint::black_box(n.wrapping_add(1))
        }
        #[inline(never)]
        extern "C" fn candidate(n: u64) -> u64 {
            std::hint::black_box(n.wrapping_add(3))
        }
        #[inline(never)]
        extern "C" fn returned(n: u64) -> u64 {
            std::hint::black_box(n.wrapping_add(7))
        }
        #[inline(never)]
        extern "C" fn getter(n: u64) -> u64 {
            unsafe { native_getter(n) }
        }

        std::arch::global_asm!(
            ".globl loyalty_owned_getter",
            "loyalty_owned_getter:",
            ".byte 0x48, 0x8d, 0x41, 0x0b",
            "ret"
        );
        unsafe extern "C" {
            #[link_name = "loyalty_owned_getter"]
            fn native_getter(n: u64) -> u64;
        }

        fn hidden_getter_worker() {
            std::thread::spawn(|| {
                let name: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
                // SAFETY: local loaded ntdll, no remote thread or game API.
                let module = unsafe { GetModuleHandleW(name.as_ptr()) };
                assert!(!module.is_null());
                let export =
                    unsafe { GetProcAddress(module, c"NtSetInformationThread".as_ptr().cast()) }
                        .unwrap();
                type SetThreadInfo =
                    unsafe extern "system" fn(HANDLE, u32, *const c_void, u32) -> i32;
                // SAFETY: ntdll export's native ABI, current thread pseudo handle.
                let set: SetThreadInfo = unsafe { std::mem::transmute(export) };
                let status = unsafe { set((-2isize) as HANDLE, 0x11, std::ptr::null(), 0) };
                assert!(status >= 0, "ThreadHideFromDebugger failed: {status:#x}");
                assert_eq!(getter(5), 16);
            })
            .join()
            .unwrap();
        }

        #[test]
        #[ignore = "only launched by the owned Windows hardware test"]
        fn owned_hardware_child() {
            let Some(path) = std::env::var_os("LOYALTY_OWNED_CHILD_HANDSHAKE") else {
                return;
            };
            // SAFETY: borrowed pseudo-handle, queried only and never closed.
            let own = identity(unsafe { GetCurrentProcess() }).unwrap();
            let addresses = [
                setter as *const () as u64,
                returned as *const () as u64,
                native_getter as *const () as u64,
                candidate as *const () as u64,
            ];
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)
                .unwrap();
            write_json(&mut file, &json!({"identity":own,"addresses":addresses})).unwrap();
            let mut go = String::new();
            std::io::stdin().read_line(&mut go).unwrap();
            assert_eq!(go.trim(), "go");
            // These workers execute the getter with no setter trigger. Their
            // real contexts must contain only slots 0/1/3, including new threads.
            std::thread::spawn(|| assert_eq!(getter(5), 16))
                .join()
                .unwrap();
            hidden_getter_worker();
            if std::env::var_os("LOYALTY_OWNED_TRACE_PRESSURE").is_some() {
                for _ in 0..140 {
                    std::thread::spawn(|| assert_eq!(getter(5), 16))
                        .join()
                        .unwrap();
                }
            }
            std::thread::spawn(|| {
                APP_UNHANDLED.store(
                    std::env::var_os("LOYALTY_OWNED_UNHANDLED").is_some(),
                    Ordering::Relaxed,
                );
                APP_EXPECT_NEXT_TF.store(
                    std::env::var_os("LOYALTY_OWNED_EXPECT_NEXT_TF").is_some(),
                    Ordering::Relaxed,
                );
                // SAFETY: callback remains loaded until explicitly removed.
                let handler = unsafe { AddVectoredExceptionHandler(1, Some(app_exception)) };
                assert!(!handler.is_null());
                let mut cache = vec![0u8; 0x1800];
                let mut raw = [0u64; 9];
                raw[7] = 0x1234;
                raw[8] = 7;
                cache[0x16E8..0x16F0].copy_from_slice(&(raw.as_ptr() as u64).to_le_bytes());
                cache[0x16F0..0x16F4].copy_from_slice(&42i32.to_le_bytes());
                let base = cache.as_ptr() as u64;
                // SAFETY: local fixture calls, valid cache allocation, Win64
                // shadow space/alignment and saved RBX. Production setter /
                // return matching and raw memory reads use actual trap context.
                let getter_result: u64;
                unsafe {
                    std::arch::asm!(
                        "push rbx", "sub rsp, 0x28", "mov rbx, rcx",
                        "lea rcx, [rbx + 0x1630]", "lea rdx, [rbx + 0x16f0]",
                        "call {setter}", "call {candidate}", "call {returned}",
                        "mov rcx, 5", "call {getter}",
                        "mov rcx, 5", "call {getter}",
                        "mov rcx, 5", "call {getter}",
                        "add rsp, 0x28", "pop rbx",
                        setter = sym setter, candidate = sym candidate,
                        returned = sym returned, getter = sym getter,
                        in("rcx") base, lateout("rax") getter_result, clobber_abi("C"),
                    );
                }
                assert_eq!(getter_result, 16);
                if std::env::var_os("LOYALTY_OWNED_BS_ONLY").is_some() {
                    // A real after-instruction TF trap, with no execution
                    // breakpoint at this local instruction. The application
                    // callback must receive BS; TF setting alone is not proof.
                    unsafe {
                        std::arch::asm!(
                            "pushfq",
                            "or qword ptr [rsp], 0x100",
                            "popfq",
                            "nop",
                            "nop"
                        );
                    }
                }
                // A new nontrigger thread, created after 0x55 was enabled on
                // its parent, must receive 0x45 rather than inherit slot 2.
                std::thread::spawn(|| assert_eq!(getter(5), 16))
                    .join()
                    .unwrap();
                hidden_getter_worker();
                // A genuine application-origin SINGLE_STEP at an unowned
                // address must reach VEH, rather than being swallowed.
                unsafe {
                    RaiseException(EXCEPTION_SINGLE_STEP as u32, 0, 0, std::ptr::null());
                }
                let extra = std::env::var_os("LOYALTY_OWNED_EXTRA_STEP").is_some();
                let report = app_report(Some(getter_result));
                let mut file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(std::env::var_os("LOYALTY_OWNED_CHILD_REPORT").unwrap())
                    .unwrap();
                write_json(&mut file, &report).unwrap();
                assert_eq!(APP_STEPS.load(Ordering::Relaxed), if extra { 2 } else { 1 });
                assert_ne!(unsafe { RemoveVectoredExceptionHandler(handler) }, 0);
                if std::env::var_os("LOYALTY_OWNED_CHILD_RESTORE").is_some() {
                    let mut finish = String::new();
                    std::io::stdin().read_line(&mut finish).unwrap();
                    assert_eq!(finish.trim(), "finish");
                }
            })
            .join()
            .unwrap();
            // No child processes are created. Let libtest retire its worker
            // thread, then the process, through real debugger exit events.
        }

        struct OwnedChild(Child);
        impl Drop for OwnedChild {
            fn drop(&mut self) {
                if self.0.try_wait().ok().flatten().is_none() {
                    let _ = self.0.kill();
                }
                // SAFETY: the very process handle retained from spawn, not a
                // PID reopen. Bound error/unwind cleanup as well as success.
                let wait = unsafe { WaitForSingleObject(self.0.as_raw_handle(), 3000) };
                assert_eq!(wait, WAIT_OBJECT_0, "owned synthetic child did not exit");
                let _ = self.0.try_wait();
            }
        }

        #[test]
        fn windows_trigger_getter_thread_and_exit() {
            run_owned_hardware(OwnedMode::Exit);
        }

        #[test]
        fn windows_trigger_getter_thread_and_restore() {
            run_owned_hardware(OwnedMode::Restore);
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        enum OwnedMode {
            Exit,
            Restore,
            AppTfObserve,
            AppTfCleanup,
            UncertainDr7,
            ReadbackMismatch,
            ReadbackError,
            AppTfReadbackError,
            AppTfReadbackMismatch,
            SetContextError,
            SecondChance,
            BsOnly,
        }

        impl OwnedMode {
            fn tf_pending(self) -> bool {
                matches!(
                    self,
                    Self::AppTfObserve
                        | Self::AppTfCleanup
                        | Self::AppTfReadbackError
                        | Self::AppTfReadbackMismatch
                )
            }

            fn context_error(self) -> bool {
                matches!(
                    self,
                    Self::ReadbackError
                        | Self::ReadbackMismatch
                        | Self::SetContextError
                        | Self::AppTfReadbackError
                        | Self::AppTfReadbackMismatch
                )
            }

            fn rf_mismatch(self) -> bool {
                matches!(self, Self::ReadbackMismatch | Self::AppTfReadbackMismatch)
            }
        }

        #[test]
        fn windows_owned_fault_with_tf_observe_delivers_next_tf_trap_under_trace_pressure() {
            run_owned_hardware(OwnedMode::AppTfObserve);
        }

        #[test]
        fn windows_owned_fault_with_tf_cleanup_delivers_next_tf_trap() {
            run_owned_hardware(OwnedMode::AppTfCleanup);
        }

        #[test]
        fn windows_uncertain_dr7_restores_before_forwarding() {
            run_owned_hardware(OwnedMode::UncertainDr7);
        }

        #[test]
        fn windows_resume_readback_mismatch_stops_and_restores() {
            run_owned_hardware(OwnedMode::ReadbackMismatch);
        }

        #[test]
        fn windows_resume_readback_error_stops_and_restores() {
            run_owned_hardware(OwnedMode::ReadbackError);
        }

        #[test]
        fn windows_tf_readback_error_keeps_owned_fault_handled_and_next_tf() {
            run_owned_hardware(OwnedMode::AppTfReadbackError);
        }

        #[test]
        fn windows_tf_readback_mismatch_keeps_owned_fault_handled_and_next_tf() {
            run_owned_hardware(OwnedMode::AppTfReadbackMismatch);
        }

        #[test]
        fn windows_set_context_error_keeps_owned_fault_handled_and_restores() {
            run_owned_hardware(OwnedMode::SetContextError);
        }

        #[test]
        fn windows_genuine_bs_only_first_chance_reaches_application() {
            run_owned_hardware(OwnedMode::BsOnly);
        }

        #[test]
        fn windows_second_chance_is_forwarded_and_child_exits() {
            run_owned_hardware(OwnedMode::SecondChance);
        }

        fn changed_rf_readback(handle: &Handle, flags: u32) -> Result<CONTEXT> {
            // Fault injection changes the OWNED CHILD'S actual OS state, not
            // the returned value/trace. Production verification must reject it.
            let mut value = context(handle, flags)?;
            value.EFlags &= !RF;
            set_context(handle, value)?;
            context(handle, flags)
        }

        fn failed_os_readback(_: &Handle, flags: u32) -> Result<CONTEXT> {
            let invalid = std::mem::ManuallyDrop::new(Handle(std::ptr::null_mut()));
            context(&invalid, flags) // genuine GetThreadContext failure, test only
        }

        fn failed_os_set(_: &Handle, value: CONTEXT) -> Result<()> {
            let invalid = std::mem::ManuallyDrop::new(Handle(std::ptr::null_mut()));
            set_context(&invalid, value)
        }

        #[test]
        fn windows_raw_b8_reads_preserve_null_and_errors() {
            let mut raw_handle = std::ptr::null_mut();
            // SAFETY: independent handle to this test process; never a live game.
            assert_ne!(
                unsafe {
                    DuplicateHandle(
                        GetCurrentProcess(),
                        GetCurrentProcess(),
                        GetCurrentProcess(),
                        &mut raw_handle,
                        0,
                        0,
                        DUPLICATE_SAME_ACCESS,
                    )
                },
                0
            );
            let handle = Handle::new(raw_handle, "raw fixture self handle").unwrap();
            let process = Process {
                pinned: identity(handle.0).unwrap(),
                handle,
            };
            let mut cache = vec![0u8; 0x1800];
            let base = cache.as_ptr() as u64;
            let trigger = Trigger {
                cache_city: base,
                receiver: base + 0x1630,
                tid: 0,
                rsp: 0,
                returned: false,
            };
            let fields = process.cache_fields(trigger);
            assert_eq!(fields["raw_receiver_plus_b8_pointer"], 0);
            assert_eq!(fields["raw_cache_plus_16e8_pointer"], 0);
            assert!(fields["raw_b8_pointee_fields"].is_null());
            cache[0x16E8..0x16F0].copy_from_slice(&1u64.to_le_bytes());
            let fields = process.cache_fields(trigger);
            assert_eq!(fields["raw_receiver_plus_b8_pointer"], 1);
            assert!(
                fields["raw_b8_pointee_fields"]["raw_pointer_plus_38_u64"]["read_error"]
                    .is_string()
            );
            assert!(
                fields["raw_b8_pointee_fields"]["raw_pointer_plus_40_u64"]["read_error"]
                    .is_string()
            );
            let fields = process.cache_fields(Trigger {
                cache_city: u64::MAX,
                receiver: u64::MAX,
                ..trigger
            });
            assert!(fields["raw_receiver_plus_b8_pointer"]["read_error"].is_string());
            assert!(fields["raw_cache_plus_16e8_pointer"]["read_error"].is_string());
            assert!(fields["raw_b8_pointee_fields"]["read_error"].is_string());
        }

        fn run_owned_hardware(mode: OwnedMode) {
            let restore_while_alive = !matches!(mode, OwnedMode::Exit | OwnedMode::BsOnly);
            let stop_at_third = !matches!(
                mode,
                OwnedMode::Exit | OwnedMode::Restore | OwnedMode::SecondChance | OwnedMode::BsOnly
            );
            let extra_step = mode.tf_pending() || mode == OwnedMode::BsOnly;
            let handshake = tempfile::NamedTempFile::new().unwrap();
            let app_report = tempfile::NamedTempFile::new().unwrap();
            let child_diagnostics = tempfile::NamedTempFile::new().unwrap();
            let child_stdout = child_diagnostics.reopen().unwrap();
            let child_stderr = child_stdout.try_clone().unwrap();
            let exe = std::env::current_exe().unwrap();
            let mut command = Command::new(&exe);
            if restore_while_alive {
                command.env("LOYALTY_OWNED_CHILD_RESTORE", "1");
            }
            if extra_step {
                command.env("LOYALTY_OWNED_EXTRA_STEP", "1");
            }
            if mode.tf_pending() {
                command.env("LOYALTY_OWNED_EXPECT_NEXT_TF", "1");
            }
            if mode == OwnedMode::BsOnly {
                command.env("LOYALTY_OWNED_BS_ONLY", "1");
            }
            if mode == OwnedMode::AppTfObserve {
                command.env("LOYALTY_OWNED_TRACE_PRESSURE", "1");
            }
            if mode == OwnedMode::SecondChance {
                command.env("LOYALTY_OWNED_UNHANDLED", "1");
            }
            let mut child = OwnedChild(
                command
                    .args([
                        "--exact",
                        "platform::live_tests::owned_hardware_child",
                        "--ignored",
                        "--nocapture",
                    ])
                    .env("LOYALTY_OWNED_CHILD_HANDSHAKE", handshake.path())
                    .env("LOYALTY_OWNED_CHILD_REPORT", app_report.path())
                    .creation_flags(CREATE_NO_WINDOW)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::from(child_stdout))
                    .stderr(Stdio::from(child_stderr))
                    .spawn()
                    .unwrap(),
            );
            let deadline = Instant::now() + Duration::from_secs(30);
            let message: Value = loop {
                if let Ok(message) =
                    serde_json::from_slice(&std::fs::read(handshake.path()).unwrap())
                {
                    break message;
                }
                assert!(Instant::now() < deadline, "child handshake deadline");
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "child exited before handshake"
                );
                std::thread::sleep(Duration::from_millis(10));
            };
            let mut raw_handle = std::ptr::null_mut();
            // SAFETY: duplicate the original spawn handle to the same kernel
            // object. No PID reopen and no unrelated process can be selected.
            assert_ne!(
                unsafe {
                    DuplicateHandle(
                        GetCurrentProcess(),
                        child.0.as_raw_handle(),
                        GetCurrentProcess(),
                        &mut raw_handle,
                        0,
                        0,
                        DUPLICATE_SAME_ACCESS,
                    )
                },
                0
            );
            let handle = Handle::new(raw_handle, "duplicate owned spawn handle").unwrap();
            let pinned = identity(handle.0).unwrap();
            assert_eq!(pinned.pid, child.0.id());
            assert_eq!(
                pinned.created,
                message["identity"]["created"].as_u64().unwrap()
            );
            assert_eq!(
                pinned.image_path,
                path_key(&canonical(&exe).unwrap()).unwrap()
            );
            assert_eq!(handle.alive(), Ok(true));
            let process = Process { handle, pinned };
            let addresses: [u64; 4] = serde_json::from_value(message["addresses"].clone()).unwrap();
            assert_eq!(addresses.iter().copied().collect::<BTreeSet<_>>().len(), 4);
            let ntdll = PinnedNtdll::open(&process).unwrap();
            let preexisting_threads = preflight_threads(&process).unwrap();
            let mut exceptions = ExceptionPolicy::default();
            exceptions.attach.bind(ntdll.verify(&process).unwrap());
            process.verify().unwrap();
            // SAFETY: own synthetic process, creation/image verified on held
            // spawn handle immediately above; no game/recorded live PID used.
            assert_ne!(unsafe { DebugActiveProcess(process.pinned.pid) }, 0);
            let mut session = Session {
                process,
                module: PinnedModule {
                    identity: ModuleIdentity {
                        path: String::new(),
                        base: 0,
                        size: 1,
                    },
                    _file: File::open(&exe).unwrap(),
                    addresses,
                },
                ntdll,
                preexisting_threads,
                attach_query_error_count: 0,
                attach_query_errors: Vec::new(),
                breakin_query_handles: BTreeMap::new(),
                threads: BTreeMap::new(),
                event_threads: BTreeMap::new(),
                debug_process: None,
                exceptions,
                owns_probes: false,
                forwarded_probe_context: false,
                probe_resume_error: None,
                cleanup_exception_records: Vec::new(),
                debug_trace: Vec::new(),
                debug_trace_omitted: 0,
                debug_trace_omitted_by_kind: BTreeMap::new(),
                module_event_seen: false,
                ready: false,
                pending: None,
                cleaned: false,
                observation: Observation::default(),
                installed_total: 0,
                detach_error: None,
            };
            // SAFETY: this test is the attaching thread.
            assert_ne!(unsafe { DebugSetProcessKillOnExit(0) }, 0);
            let output = tempfile::NamedTempFile::new().unwrap();
            let mut output_file = output.reopen().unwrap();
            let args = Args {
                pid: session.process.pinned.pid,
                created: session.process.pinned.created,
                output: output.path().to_owned(),
                seconds: 20,
            };
            let mut exits = 0;
            let mut process_exit = None;
            let mut getter_traps = 0;
            let mut terminal_reason = None;
            for _ in 0..2048 {
                assert!(Instant::now() < deadline, "real hardware event deadline");
                let mut event = DEBUG_EVENT::default();
                // SAFETY: bounded wait on the attaching thread.
                if unsafe { WaitForDebugEvent(&mut event, 100) } == 0 {
                    assert_eq!(unsafe { GetLastError() }, ERROR_SEM_TIMEOUT);
                    continue;
                }
                session.pending = Some(Pending {
                    event,
                    restore_before_continue: false,
                    exception_routed: false,
                    decision: None,
                    status: if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT {
                        DBG_EXCEPTION_NOT_HANDLED
                    } else {
                        DBG_CONTINUE
                    },
                });
                if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT && !session.ready {
                    // Exercise production attach identity/routing/installation;
                    // only Civ6 DLL verification and ready-file publication
                    // are excluded for this executable fixture.
                    let info = unsafe { event.u.Exception };
                    let raw = RawException {
                        tid: event.dwThreadId,
                        code: info.ExceptionRecord.ExceptionCode as u32,
                        first_chance: info.dwFirstChance,
                        exception_address: info.ExceptionRecord.ExceptionAddress as u64,
                    };
                    assert_eq!(
                        session.route_exception(raw, None).unwrap(),
                        ExceptionDecision::AttachBreak
                    );
                    let tids: Vec<_> = session.threads.keys().copied().collect();
                    for tid in tids {
                        session.install(tid).unwrap();
                    }
                    session.ready = true;
                    child.0.stdin.as_mut().unwrap().write_all(b"go\n").unwrap();
                } else {
                    if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT {
                        let info = unsafe { event.u.Exception };
                        if info.ExceptionRecord.ExceptionCode == EXCEPTION_SINGLE_STEP
                            && info.dwFirstChance != 0
                            && info.ExceptionRecord.ExceptionAddress as u64 == addresses[2]
                        {
                            getter_traps += 1;
                            if getter_traps == 3 && stop_at_third {
                                let thread = session.threads.get(&event.dwThreadId).unwrap();
                                let mut value = context(
                                    &thread.handle,
                                    CONTEXT_DEBUG_REGISTERS_AMD64
                                        | CONTEXT_CONTROL_AMD64
                                        | CONTEXT_INTEGER_AMD64,
                                )
                                .unwrap();
                                if mode.tf_pending() {
                                    println!(
                                        "OWNED_FAULT_CONTEXT_BEFORE_TF_SETTING={}",
                                        json!({"dr6":value.Dr6,"eflags":value.EFlags,"rip":value.Rip})
                                    );
                                    value.ContextFlags = CONTEXT_CONTROL_AMD64;
                                    value.EFlags |= 0x100;
                                    set_context(&thread.handle, value).unwrap();
                                    let actual = context(
                                        &thread.handle,
                                        CONTEXT_CONTROL_AMD64 | CONTEXT_DEBUG_REGISTERS_AMD64,
                                    )
                                    .unwrap();
                                    assert_eq!(actual.Dr6 & 0xF, 4);
                                    assert_ne!(actual.EFlags & 0x100, 0);
                                    value = actual;
                                    if mode == OwnedMode::AppTfCleanup {
                                        break;
                                    }
                                }
                                if mode == OwnedMode::UncertainDr7 {
                                    value.Dr7 &= !0x10;
                                    set_context(&thread.handle, value).unwrap();
                                } else if mode.context_error() {
                                    let raw = RawException {
                                        tid: event.dwThreadId,
                                        code: EXCEPTION_SINGLE_STEP as u32,
                                        first_chance: 1,
                                        exception_address: addresses[2],
                                    };
                                    let decision = session
                                        .route_exception(
                                            raw,
                                            Some(ProbeSnapshot {
                                                armed: true,
                                                registers: registers(&value),
                                                rip: value.Rip,
                                                flags: value.EFlags,
                                            }),
                                        )
                                        .unwrap();
                                    let readback = if mode.rf_mismatch() {
                                        changed_rf_readback
                                    } else if mode == OwnedMode::SetContextError {
                                        context
                                    } else {
                                        failed_os_readback
                                    };
                                    let failure = session
                                        .resume_probe_with_context_io(
                                            raw,
                                            decision,
                                            value,
                                            "test_readback_fault",
                                            if mode == OwnedMode::SetContextError {
                                                failed_os_set
                                            } else {
                                                set_context
                                            },
                                            readback,
                                        )
                                        .unwrap_err();
                                    assert!(
                                        failure.contains(if mode.rf_mismatch() {
                                            "mismatch: RF"
                                        } else if mode == OwnedMode::SetContextError {
                                            "SetThreadContext"
                                        } else {
                                            "readback failed"
                                        }),
                                        "{failure}"
                                    );
                                    assert!(session
                                        .continue_pending()
                                        .unwrap_err()
                                        .contains("centralized probe restoration"));
                                    assert_eq!(
                                        session.pending.as_ref().unwrap().status,
                                        DBG_CONTINUE
                                    );
                                    assert_eq!(
                                        session.probe_resume_error.as_deref(),
                                        Some(failure.as_str())
                                    );
                                    break;
                                }
                            }
                        }
                    }
                    let reason = session
                        .event(&args, &mut output_file, &event)
                        .unwrap_or_else(|error| {
                            println!("FAILED_HARDWARE_TRACE={}", json!(session.debug_trace));
                            panic!("{error}");
                        });
                    if event.dwDebugEventCode == EXIT_THREAD_DEBUG_EVENT {
                        exits += 1;
                    }
                    if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
                        process_exit = Some(unsafe { event.u.ExitProcess.dwExitCode });
                        assert_eq!(reason, Some("process_exited"));
                        break; // finish must retire this pending exit before freezing
                    }
                    if reason.is_some() {
                        assert_eq!(
                            reason,
                            Some(match mode {
                                OwnedMode::AppTfObserve => "app_tf_pending_after_owned_fault",
                                OwnedMode::UncertainDr7 => "owned_probe_identity_uncertain",
                                OwnedMode::SecondChance => "second_chance_exception",
                                _ => panic!("unexpected terminal event: {mode:?} {reason:?}"),
                            })
                        );
                        terminal_reason = reason;
                        break;
                    }
                }
                session.continue_pending().unwrap();
                if mode == OwnedMode::Restore && session.exceptions.unknown_first_chance_count == 1
                {
                    break;
                }
                if event.dwDebugEventCode == EXIT_THREAD_DEBUG_EVENT {
                    assert!(!session.threads.contains_key(&event.dwThreadId));
                    assert!(!session.event_threads.contains_key(&event.dwThreadId));
                }
            }
            let report = session.finish();
            if mode == OwnedMode::UncertainDr7 {
                // The current event has no trustworthy ownership proof. Keep
                // its original NOT_HANDLED continuation. API restoration alone
                // cannot prove the application's exception-context delivery.
                assert!(session.forwarded_probe_context);
            } else {
                assert!(report.complete(), "{report:?}");
            }
            if matches!(mode, OwnedMode::SecondChance | OwnedMode::UncertainDr7) {
                assert_eq!(
                    terminal_reason,
                    Some(if mode == OwnedMode::SecondChance {
                        "second_chance_exception"
                    } else {
                        "owned_probe_identity_uncertain"
                    })
                );
                assert_eq!(
                    unsafe { WaitForSingleObject(session.process.handle.0, 3000) },
                    WAIT_OBJECT_0
                );
                assert_eq!(
                    child
                        .0
                        .try_wait()
                        .unwrap()
                        .unwrap()
                        .code()
                        .map(|c| c as u32),
                    Some(0x80000004)
                );
                assert_eq!(
                    session.exceptions.second_chance_count,
                    u64::from(mode == OwnedMode::SecondChance)
                );
            } else if restore_while_alive {
                assert_eq!(process_exit, None);
                assert!(!report.process_exited);
                assert!(report.threads_restored >= 2, "{report:?}");
                assert_eq!(session.process.handle.alive(), Ok(true));
                child
                    .0
                    .stdin
                    .as_mut()
                    .unwrap()
                    .write_all(b"finish\n")
                    .unwrap();
                let wait = unsafe { WaitForSingleObject(session.process.handle.0, 3000) };
                if wait != WAIT_OBJECT_0 {
                    for (&tid, thread) in &session.threads {
                        if thread.handle.alive() == Ok(true) {
                            let previous = unsafe { SuspendThread(thread.handle.0) };
                            let value = context(&thread.handle, CONTEXT_CONTROL_AMD64);
                            println!(
                                "CHILD_THREAD_STATE={}",
                                json!({"tid":tid,"suspend_count":previous,"context":value.map(|v| json!({"rip":v.Rip,"eflags":v.EFlags,"rsp":v.Rsp})),"owned_suspend":thread.suspended})
                            );
                            if previous != u32::MAX {
                                unsafe { ResumeThread(thread.handle.0) };
                            }
                        }
                    }
                    println!("FAILED_HARDWARE_TRACE={}", json!(session.debug_trace));
                    println!(
                        "CHILD_DIAGNOSTICS={}",
                        std::fs::read_to_string(child_diagnostics.path()).unwrap()
                    );
                    println!(
                        "CHILD_APP_REPORT={}",
                        std::fs::read_to_string(app_report.path()).unwrap()
                    );
                }
                assert_eq!(wait, WAIT_OBJECT_0);
                assert!(child.0.try_wait().unwrap().unwrap().success());
            } else {
                assert_eq!(process_exit, Some(0));
                assert!(exits >= 2);
                assert!(report.process_exited);
                assert_eq!(session.process.handle.alive(), Ok(false));
            }
            let app: Value =
                serde_json::from_slice(&std::fs::read(app_report.path()).unwrap()).unwrap();
            let trace = json!({"identity":session.process.pinned,"process_exit":process_exit,
                "mode":format!("{mode:?}"),"app":app,"child_exit_confirmed":true,
                "forwarded_probe_context_delivery_unverified":session.forwarded_probe_context,
                "probe_resume_error":session.probe_resume_error,
                "fixture_nontrigger_getter_calls":if mode == OwnedMode::AppTfObserve { 144 } else { 4 },
                "fixture_hidden_getter_calls":2,
                "thread_exit_events":exits,"cleanup":report,"trace":session.debug_trace,
                "trace_omitted":session.debug_trace_omitted,
                "trace_omitted_by_kind":session.debug_trace_omitted_by_kind});
            println!("LIVE_HARDWARE_TRACE={trace}");
            let mut counts = [0; 4];
            for record in &session.debug_trace {
                if record["event"] == "probe_resume" {
                    if let Some(slot) = record["decision"]["Probe"]["slot"]
                        .as_u64()
                        .or_else(|| record["decision"]["ProbeAndStopForAppTF"]["slot"].as_u64())
                    {
                        counts[slot as usize] += 1;
                    }
                    if record["phase"] == "test_readback_fault" {
                        assert_eq!(record["continue_status"], DBG_CONTINUE);
                        assert_eq!(record["set_context_ok"], mode != OwnedMode::SetContextError);
                        assert_eq!(record["readback_ok"], false);
                        if mode.rf_mismatch() {
                            assert_eq!(
                                record["actual"]["eflags"].as_u64().unwrap() & u64::from(RF),
                                0
                            );
                        } else if mode == OwnedMode::SetContextError {
                            assert!(record["set_context_error"]
                                .as_str()
                                .unwrap()
                                .contains("SetThreadContext"));
                            assert_eq!(record["actual"]["dr6"].as_u64().unwrap() & 0xF, 4);
                        } else {
                            assert!(record["actual"]["read_error"].is_string());
                        }
                        continue;
                    }
                    assert_eq!(record["set_context_ok"], true);
                    assert_eq!(record["readback_ok"], true);
                    assert!(record["actual"]["eflags"].as_u64().is_some(), "{record}");
                    assert_ne!(
                        record["actual"]["eflags"].as_u64().unwrap() & u64::from(RF),
                        0
                    );
                    assert_eq!(record["actual"]["dr6"].as_u64().unwrap() & 0xF, 0);
                    assert_eq!(record["actual"]["rip"], record["before"]["rip"]);
                    assert_eq!(
                        record["continue_status"],
                        if record["decision"]["Probe"].is_object()
                            || record["decision"]["ProbeAndStopForAppTF"].is_object()
                        {
                            DBG_CONTINUE
                        } else {
                            DBG_EXCEPTION_NOT_HANDLED
                        }
                    );
                } else if record["event"] == "debug_continue" {
                    assert_eq!(record["ok"], true);
                } else if record["event"] == "thread_resume" {
                    assert_eq!(record["ok"], true);
                    assert_eq!(record["previous_count"], 1);
                    assert_eq!(record["runnable"], true);
                }
            }
            let lost_getter = mode == OwnedMode::UncertainDr7;
            assert_eq!(counts, [1, 1, if lost_getter { 2 } else { 3 }, 1]);
            let trigger = session.observation.trigger.unwrap();
            assert!(trigger.returned);
            assert_eq!(
                session.observation.getters_recorded,
                if stop_at_third { 2 } else { 3 }
            );
            let installs: Vec<_> = session
                .debug_trace
                .iter()
                .filter(|r| r["event"] == "probe_install")
                .collect();
            assert!(installs.len() >= 5);
            for install in installs {
                assert_eq!(install["actual"]["dr7"].as_u64().unwrap() & !0x400, 0x45);
                assert_eq!(install["actual"]["address"][2], 0);
            }
            let enables: Vec<_> = session
                .debug_trace
                .iter()
                .filter(|r| r["event"] == "trigger_getter_enable")
                .collect();
            assert_eq!(enables.len(), 1);
            assert_eq!(enables[0]["tid"], trigger.tid);
            assert_eq!(enables[0]["actual"]["dr7"].as_u64().unwrap() & !0x400, 0x55);
            for record in session
                .debug_trace
                .iter()
                .filter(|r| r["decision"]["Probe"]["slot"] == 2)
            {
                assert_eq!(record["raw"]["tid"], trigger.tid);
            }
            let samples: Vec<Value> =
                std::io::BufRead::lines(std::io::BufReader::new(output.reopen().unwrap()))
                    .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
                    .collect();
            for event in ["setter_before", "setter_return"] {
                let sample = samples.iter().find(|r| r["event"] == event).unwrap();
                let fields = &sample["fields"];
                assert_eq!(
                    fields["raw_receiver_plus_b8_pointer"],
                    fields["raw_cache_plus_16e8_pointer"]
                );
                assert!(fields["raw_receiver_plus_b8_pointer"].as_u64().unwrap() != 0);
                assert_eq!(
                    fields["raw_b8_pointee_fields"]["raw_pointer_plus_38_u64"],
                    0x1234
                );
                assert_eq!(
                    fields["raw_b8_pointee_fields"]["raw_pointer_plus_40_u64"],
                    7
                );
            }
            assert_eq!(
                samples
                    .iter()
                    .filter(|r| r["event"] == "getter_raw")
                    .count(),
                if stop_at_third { 2 } else { 3 }
            );
            for sample in samples.iter().filter(|r| r["event"] == "getter_raw") {
                assert_eq!(sample["tid"], trigger.tid);
                assert_eq!(sample["city_identity_verified"], false);
            }
            if mode == OwnedMode::AppTfObserve {
                assert_eq!(session.debug_trace.len(), 128);
                assert!(session.installed_total > 140);
                assert!(session.debug_trace_omitted_by_kind["probe_install/capacity"] > 0);
                assert!(
                    session.debug_trace_omitted_by_kind["probe_install/replaced_by_priority"] > 0
                );
                assert!(session.debug_trace.iter().any(|r| r["event"] == "exit"));
            } else {
                assert_eq!(session.debug_trace_omitted, 0);
            }
            if mode.tf_pending() {
                assert_eq!(app["app_steps"], 2);
                assert_eq!(
                    app["first_dr6"].as_u64().unwrap() & 0xE00F,
                    0,
                    "following TF trap must contain no owned B/BD/BT bits: {app}"
                );
                assert_eq!(app["first_dr7"].as_u64().unwrap() & !0x400, 0);
                assert_eq!(app["first_code"], EXCEPTION_SINGLE_STEP as u32);
                assert_eq!(app["first_address"], app["first_rip"]);
                assert_eq!(app["first_rip"], addresses[2] + 4);
                assert_eq!(app["events"][0]["next_tf_instruction_boundary"], true);
                assert_eq!(app["app_tf_handled"], 1);
                assert_eq!(app["normal_result"], 16);
                let resumed = session
                    .debug_trace
                    .iter()
                    .find(|r| r["decision"]["ProbeAndStopForAppTF"].is_object())
                    .unwrap();
                assert_eq!(
                    resumed["phase"],
                    if mode == OwnedMode::AppTfCleanup {
                        "cleanup"
                    } else if mode.context_error() {
                        "test_readback_fault"
                    } else {
                        "observe"
                    }
                );
                if mode != OwnedMode::AppTfReadbackError {
                    assert_eq!(resumed["actual"]["dr6"].as_u64().unwrap() & 0xE00F, 0);
                    assert_ne!(resumed["actual"]["eflags"].as_u64().unwrap() & 0x100, 0);
                } else {
                    assert!(resumed["actual"]["read_error"].is_string());
                }
                assert_eq!(resumed["continue_status"], DBG_CONTINUE);
                let tf_resume_index = session
                    .debug_trace
                    .iter()
                    .position(|r| r["decision"]["ProbeAndStopForAppTF"].is_object())
                    .unwrap();
                let owned_continue = session
                    .debug_trace
                    .iter()
                    .enumerate()
                    .skip(tf_resume_index + 1)
                    .find(|(_, r)| {
                        r["event"] == "debug_continue"
                            && r["raw"]["exception_address"] == addresses[2]
                    })
                    .map(|(i, r)| {
                        assert_eq!(r["continue_status"], DBG_CONTINUE);
                        i
                    })
                    .unwrap();
                let restores: Vec<_> = session
                    .debug_trace
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r["event"] == "probe_restore")
                    .collect();
                assert_eq!(restores.len(), report.threads_restored as usize);
                assert!(restores
                    .iter()
                    .all(|(index, r)| *index < owned_continue && r["readback_ok"] == true));
                for (_, restored) in restores {
                    assert_eq!(restored["actual"]["dr7"].as_u64().unwrap() & !0x400, 0);
                    if restored["tid"] == resumed["raw"]["tid"] {
                        assert_ne!(restored["actual"]["eflags"].as_u64().unwrap() & 0x100, 0);
                        assert_eq!(restored["actual"]["rip"], resumed["before"]["rip"]);
                    }
                }
                assert!(!session.forwarded_probe_context);
            } else if mode == OwnedMode::UncertainDr7 {
                assert_eq!(app["app_steps"], 1);
                assert_eq!(app["first_dr6"].as_u64().unwrap() & 0xF, 4);
                let last_restore = session
                    .debug_trace
                    .iter()
                    .rposition(|r| r["event"] == "probe_restore")
                    .unwrap();
                let forwarded = session
                    .debug_trace
                    .iter()
                    .position(|r| {
                        r["event"] == "debug_continue"
                            && r["continue_status"] == DBG_EXCEPTION_NOT_HANDLED
                    })
                    .unwrap();
                assert!(last_restore < forwarded);
            }
            if mode.context_error() {
                assert!(session.probe_resume_error.is_some());
                assert!(!session.forwarded_probe_context);
                assert_eq!(app["app_steps"], if mode.tf_pending() { 2 } else { 1 });
                assert_eq!(app["first_dr6"].as_u64().unwrap() & 0xE00F, 0);
                assert_eq!(app["first_dr7"].as_u64().unwrap() & !0x400, 0);
                assert_ne!(app["first_rip"], addresses[2]);
                assert_eq!(app["normal_result"], 16);
                let fault = session
                    .debug_trace
                    .iter()
                    .find(|r| r["phase"] == "test_readback_fault")
                    .unwrap();
                let continuations: Vec<_> = session
                    .debug_trace
                    .iter()
                    .filter(|r| {
                        r["event"] == "debug_continue"
                            && r["raw"]["exception_address"] == addresses[2]
                    })
                    .collect();
                assert_eq!(continuations.len(), 3);
                assert!(continuations
                    .iter()
                    .all(|r| r["continue_status"] == DBG_CONTINUE));
                assert_eq!(fault["continue_status"], DBG_CONTINUE);
                let fault_index = session
                    .debug_trace
                    .iter()
                    .position(|r| r["phase"] == "test_readback_fault")
                    .unwrap();
                let after_fault: Vec<_> = session
                    .debug_trace
                    .iter()
                    .enumerate()
                    .skip(fault_index + 1)
                    .filter(|(_, r)| {
                        r["event"] == "debug_continue"
                            && r["raw"]["exception_address"] == addresses[2]
                    })
                    .collect();
                assert_eq!(after_fault.len(), 1);
                let restored: Vec<_> = session
                    .debug_trace
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r["event"] == "probe_restore")
                    .collect();
                assert_eq!(restored.len(), report.threads_restored as usize);
                assert!(restored.iter().all(|(i, r)| *i > fault_index
                    && *i < after_fault[0].0
                    && r["readback_ok"] == true));
                assert_eq!(
                    fault["set_context_error"]
                        .as_str()
                        .or(fault["readback_error"].as_str()),
                    session.probe_resume_error.as_deref()
                );
            }
            if mode != OwnedMode::UncertainDr7 && mode != OwnedMode::SecondChance {
                assert!(
                    app["events"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|e| e["leaked_owned_fault"] == false),
                    "{app}"
                );
            }
            if mode == OwnedMode::BsOnly {
                assert_eq!(app["app_steps"], 2);
                assert_eq!(app["first_dr6"].as_u64().unwrap() & 0xE00F, 0x4000);
                assert_eq!(app["app_tf_handled"], 1);
            }
            let forwarded = session.debug_trace.iter().find(|record| {
                record["event"] == "debug_continue"
                    && record["continue_status"] == DBG_EXCEPTION_NOT_HANDLED
            });
            if mode.context_error() || mode.tf_pending() {
                assert!(forwarded.is_none()); // the handled fault is the final debugger event
            } else {
                let forwarded = forwarded.unwrap();
                assert_eq!(forwarded["raw"]["code"], EXCEPTION_SINGLE_STEP as u32);
                assert_eq!(forwarded["raw"]["first_chance"], 1);
            }
            assert_eq!(
                session.exceptions.unknown_first_chance_count,
                if mode.context_error() || mode.tf_pending() {
                    0
                } else if mode == OwnedMode::BsOnly {
                    2
                } else {
                    1
                }
            );
            assert_eq!(
                session.exceptions.second_chance_count,
                u64::from(mode == OwnedMode::SecondChance)
            );
            for continued in session
                .debug_trace
                .iter()
                .filter(|r| r["event"] == "debug_continue" && r["raw"]["first_chance"] == 0)
            {
                assert_eq!(continued["continue_status"], DBG_EXCEPTION_NOT_HANDLED);
            }
        }
    }
}

fn main() {
    let args = Args::parse();
    #[cfg(all(windows, target_arch = "x86_64"))]
    let result = platform::run(&args);
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    let result: Result<()> = {
        let _ = args;
        Err("loyalty-native-observer CLI is available only on x64 Windows".into())
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::attach_image::*;
    use super::{policy::*, *};
    use std::cell::RefCell;

    fn pe_fixture() -> Vec<u8> {
        let mut bytes = vec![0; 0xA00];
        fn put16(bytes: &mut [u8], offset: usize, value: u16) {
            bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        fn put32(bytes: &mut [u8], offset: usize, value: u32) {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        put16(&mut bytes, 0, 0x5A4D);
        put32(&mut bytes, 0x3C, 0x80);
        put32(&mut bytes, 0x80, 0x4550);
        put16(&mut bytes, 0x84, 0x8664);
        put16(&mut bytes, 0x86, 2);
        put16(&mut bytes, 0x94, 240);
        put16(&mut bytes, 0x96, 0x2022);
        let opt = 0x98;
        put16(&mut bytes, opt, 0x20B);
        put32(&mut bytes, opt + 56, 0x5000);
        put32(&mut bytes, opt + 60, 0x200);
        put32(&mut bytes, opt + 108, 16);
        put32(&mut bytes, opt + 112, 0x3000);
        put32(&mut bytes, opt + 116, 0x200);
        for (index, va, raw, flags) in [
            (0, 0x1000, 0x200, 0x60000020),
            (1, 0x3000, 0x600, 0x40000040),
        ] {
            let section = 0x188 + index * 40;
            put32(&mut bytes, section + 8, 0x400);
            put32(&mut bytes, section + 12, va);
            put32(&mut bytes, section + 16, 0x400);
            put32(&mut bytes, section + 20, raw);
            put32(&mut bytes, section + 36, flags);
        }
        put32(&mut bytes, 0x600 + 20, 3);
        put32(&mut bytes, 0x600 + 24, 3);
        put32(&mut bytes, 0x600 + 28, 0x3040);
        put32(&mut bytes, 0x600 + 32, 0x3060);
        put32(&mut bytes, 0x600 + 36, 0x3080);
        let mut string_offset = 0x6A0;
        for (index, name) in EXPORT_NAMES.iter().enumerate() {
            put32(&mut bytes, 0x640 + index * 4, 0x1010 + index as u32 * 0x20);
            put32(
                &mut bytes,
                0x660 + index * 4,
                0x3000 + (string_offset - 0x600) as u32,
            );
            put16(&mut bytes, 0x680 + index * 2, index as u16);
            bytes[string_offset..string_offset + name.len()].copy_from_slice(name.as_bytes());
            string_offset += name.len() + 1;
        }
        bytes[0x210..0x218].copy_from_slice(&[0xCC, 0xC3, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90]);
        bytes[0x230..0x238].copy_from_slice(&[0x48, 0x83, 0xEC, 0x38, 0x48, 0x8B, 0x05, 0x01]);
        bytes[0x250..0x258].copy_from_slice(&[0x4C, 0x8B, 0xD1, 0xB8, 0x25, 0, 0, 0]);
        bytes
    }

    struct FixtureReader {
        base: u64,
        bytes: Vec<u8>,
        calls: RefCell<Vec<(u64, usize)>>,
    }
    impl FixtureReader {
        fn new(file: &[u8], base: u64) -> Self {
            let mut bytes = vec![0; 0x5000];
            bytes[..0x200].copy_from_slice(&file[..0x200]);
            bytes[0x1000..0x1400].copy_from_slice(&file[0x200..0x600]);
            bytes[0x3000..0x3400].copy_from_slice(&file[0x600..0xA00]);
            Self {
                base,
                bytes,
                calls: RefCell::new(Vec::new()),
            }
        }
    }
    impl ShortImageReader for FixtureReader {
        fn read_field(&self, address: u64, length: usize) -> Result<Vec<u8>> {
            assert!(
                (1..=8).contains(&length),
                "must never turn identity checks into a dump"
            );
            self.calls.borrow_mut().push((address, length));
            let offset = address
                .checked_sub(self.base)
                .ok_or("mock wrong image base")? as usize;
            self.bytes
                .get(offset..offset.checked_add(length).ok_or("mock field overflow")?)
                .map(|bytes| bytes.to_vec())
                .ok_or_else(|| "mock inaccessible PE field".into())
        }
    }

    fn verified_fixture() -> VerifiedAttachExports {
        let file = pe_fixture();
        let image = PeImage::parse(&file).unwrap();
        let reader = FixtureReader::new(&file, 0x7FFF00000000);
        image.verify_loaded(&reader, reader.base, 0x5000).unwrap()
    }

    fn breakpoint(tid: u32, address: u64) -> RawException {
        RawException {
            tid,
            code: 0x80000003,
            first_chance: 1,
            exception_address: address,
        }
    }

    fn identity() -> Identity {
        Identity {
            pid: 123,
            created: 0x0123456789abcdef,
            image_path: "pinned-game-image".into(),
        }
    }

    #[test]
    fn pid_reuse_exit_and_image_swap_are_rejected() {
        let pinned = identity();
        assert!(match_identity(&pinned, &pinned, true).is_ok());
        assert!(match_identity(&pinned, &pinned, false).is_err());
        for other in [
            Identity {
                pid: 124,
                ..pinned.clone()
            },
            Identity {
                created: pinned.created + 1,
                ..pinned.clone()
            },
            Identity {
                image_path: "another-image".into(),
                ..pinned.clone()
            },
        ] {
            assert!(match_identity(&pinned, &other, true).is_err());
        }
    }

    #[test]
    fn image_layout_and_aslr_range_are_pinned() {
        for name in ["CivilizationVI.exe", "CivilizationVI_DX12.exe"] {
            let image = PathBuf::from("game/Base/Binaries/Win64Steam").join(name);
            assert_eq!(
                expected_module(&image).unwrap(),
                PathBuf::from("game/DLC/Expansion2/Binaries/Win64").join(MODULE_NAME)
            );
        }
        assert!(expected_module(Path::new("game/Base/Binaries/Win64Steam/Other.exe")).is_err());
        assert!(expected_module(Path::new("game/CivilizationVI.exe")).is_err());
        assert!(breakpoint_addresses(u64::MAX - 1, u32::MAX).is_err());
        assert!(breakpoint_addresses(0x180000000, RVAS[0] as u32).is_err());
        let base = 0x7ffd00000000;
        assert_eq!(
            breakpoint_addresses(base, 0x800000).unwrap(),
            RVAS.map(|rva| base + rva)
        );
        assert_eq!(MODULE_SHA256.len(), 64);
    }

    #[test]
    fn only_unused_debug_registers_can_be_overlaid() {
        for original in [
            DebugRegisters::default(),
            DebugRegisters {
                dr6: 0xFFFF0FF0,
                dr7: 0x400,
                ..Default::default()
            },
        ] {
            assert!(original.unused());
            let installed = original.installed([11, 22, 33, 44], 0x55);
            assert_eq!(installed.dr6, original.dr6);
            assert_eq!(installed.dr7 & !0x400, 0x55);
        }
        for registers in [
            DebugRegisters {
                address: [0, 0, 1, 0],
                ..Default::default()
            },
            DebugRegisters {
                dr6: 1,
                ..Default::default()
            },
            DebugRegisters {
                dr6: 0x4000,
                ..Default::default()
            },
            DebugRegisters {
                dr7: 1,
                ..Default::default()
            },
            DebugRegisters {
                dr7: 1 << 16,
                ..Default::default()
            },
        ] {
            assert!(!registers.unused());
        }
    }

    #[test]
    fn hardware_hits_select_rip_slot_and_separate_application_sources() {
        let addresses = [11, 22, 33, 44];
        let installed = DebugRegisters::default().installed(addresses, 0x55);
        for slot in 0..4 {
            let hit = DebugRegisters {
                dr6: 1 << slot,
                ..installed
            };
            assert_eq!(
                hit.own_hit(addresses, 0x55, addresses[slot], 0),
                Some(probe_source(slot, 1 << slot))
            );
            assert_eq!(
                hit.own_hit(addresses, 0x55, addresses[(slot + 1) % 4], 0),
                None
            );
            assert_eq!(
                hit.own_hit(addresses, 0x55, addresses[slot], 0x100),
                Some(ProbeSource {
                    app_tf: true,
                    ..probe_source(slot, 1 << slot)
                })
            );
            assert_eq!(
                DebugRegisters {
                    dr6: hit.dr6 | 0x4000,
                    ..hit
                }
                .own_hit(addresses, 0x55, addresses[slot], 0),
                Some(ProbeSource {
                    app_dr6: 0x4000,
                    ..probe_source(slot, 1 << slot)
                })
            );
        }
        assert_eq!(
            DebugRegisters {
                dr6: 3,
                ..installed
            }
            .own_hit(addresses, 0x55, 11, 0),
            Some(probe_source(0, 3))
        );
        assert_eq!(installed.own_hit(addresses, 0x55, 11, 0), None);
        assert_eq!(
            DebugRegisters {
                dr6: 1,
                dr7: 0,
                ..installed
            }
            .own_hit(addresses, 0x55, 11, 0),
            None
        );
        assert_eq!(RF, 0x10000);
    }

    fn probe_source(slot: usize, owned_b_bits: u64) -> ProbeSource {
        ProbeSource {
            slot,
            owned_b_bits,
            app_dr6: 0,
            app_tf: false,
        }
    }

    #[test]
    fn trigger_mask_disabled_getter_and_composite_step_are_forwarded() {
        let addresses = [11, 22, 33, 44];
        for enabled in [0x45, 0x55] {
            let installed = DebugRegisters::default().installed(addresses, enabled);
            assert_eq!(installed.address[2], if enabled == 0x45 { 0 } else { 33 });
            for slot in 0..4 {
                let exception = RawException {
                    tid: 9,
                    code: 0x80000004,
                    first_chance: 1,
                    exception_address: addresses[slot],
                };
                for (dr6, flags) in [
                    (1 << slot, 0x202),
                    (1 << slot, 0x302),
                    ((1 << slot) | 0x2000, 0x202),
                    ((1 << slot) | 0x4000, 0x202),
                    ((1 << slot) | 0x8000, 0x202),
                    (0, 0x202),
                ] {
                    let snapshot = ProbeSnapshot {
                        armed: true,
                        registers: DebugRegisters { dr6, ..installed },
                        rip: addresses[slot],
                        flags,
                    };
                    let mut policy = ExceptionPolicy::default();
                    let decision = policy.decide(exception, Some(snapshot), addresses, enabled);
                    if enabled & (1 << (slot * 2)) != 0 && dr6 & (1 << slot) != 0 {
                        let source = ProbeSource {
                            app_dr6: dr6 & 0xE000,
                            app_tf: flags & 0x100 != 0,
                            ..probe_source(slot, 1 << slot)
                        };
                        if source.app_dr6 == 0 && source.app_tf {
                            assert_eq!(decision, ExceptionDecision::ProbeAndStopForAppTF(source));
                            assert!(decision.handled());
                            assert!(decision.stop_observing());
                        } else if source.app_dr6 == 0 {
                            assert_eq!(decision, ExceptionDecision::Probe(source));
                        } else {
                            assert_eq!(decision, ExceptionDecision::ForwardComposite(source));
                            assert!(!decision.handled());
                            assert!(decision.stop_observing());
                        }
                        assert_eq!(
                            decision.resume_debug_flags(dr6, flags),
                            Some((dr6 & !(1 << slot), flags | RF))
                        );
                    } else if enabled & (1 << (slot * 2)) != 0 {
                        assert_eq!(decision, ExceptionDecision::RestoreAndForward);
                        assert_eq!(decision.resume_debug_flags(dr6, flags), None);
                    } else {
                        assert_eq!(
                            decision,
                            ExceptionDecision::ForwardFirstChance { record: true }
                        );
                        assert_eq!(decision.resume_debug_flags(dr6, flags), None);
                    }
                }
                // No snapshot, including cleanup context failure, cannot turn
                // even an enabled address into evidence of an owned exception.
                assert_eq!(
                    ExceptionPolicy::default().decide(exception, None, addresses, enabled),
                    ExceptionDecision::RestoreAndForward
                );
            }
        }
    }

    #[test]
    fn tf_setting_is_not_a_current_bs_source_and_ownership_stays_exact() {
        let addresses = [11, 22, 33, 44];
        let mut policy = ExceptionPolicy::default();
        for enabled in [0x45, 0x55] {
            for slot in 0..4 {
                if enabled & (1 << (slot * 2)) == 0 {
                    continue;
                }
                let exception = RawException {
                    tid: 9,
                    code: 0x80000004,
                    first_chance: 1,
                    exception_address: addresses[slot],
                };
                let source = ProbeSource {
                    app_tf: true,
                    ..probe_source(slot, 1 << slot)
                };
                let snapshot = ProbeSnapshot {
                    armed: true,
                    flags: 0x302,
                    rip: addresses[slot],
                    registers: DebugRegisters {
                        dr6: (1 << slot) | 0xFFFF0FF0,
                        ..DebugRegisters::default().installed(addresses, enabled)
                    },
                };
                let decision = policy.decide(exception, Some(snapshot), addresses, enabled);
                assert_eq!(decision, ExceptionDecision::ProbeAndStopForAppTF(source));
                assert!(decision.handled());
                assert!(decision.stop_observing());
                assert_eq!(
                    decision.resume_debug_flags(snapshot.registers.dr6, snapshot.flags),
                    Some((0xFFFF0FF0, 0x10302))
                );
                for app_source in [0x2000, 0x4000, 0x8000] {
                    let true_mixed = ProbeSnapshot {
                        registers: DebugRegisters {
                            dr6: snapshot.registers.dr6 | app_source,
                            ..snapshot.registers
                        },
                        ..snapshot
                    };
                    let decision = policy.decide(exception, Some(true_mixed), addresses, enabled);
                    assert_eq!(
                        decision,
                        ExceptionDecision::ForwardComposite(ProbeSource {
                            app_dr6: app_source,
                            ..source
                        })
                    );
                    assert!(!decision.handled());
                    assert_eq!(
                        decision.resume_debug_flags(true_mixed.registers.dr6, true_mixed.flags),
                        Some((0xFFFF0FF0 | app_source, 0x10302))
                    );
                }
                assert_eq!(
                    policy.decide(
                        RawException {
                            exception_address: 99,
                            ..exception
                        },
                        Some(snapshot),
                        addresses,
                        enabled
                    ),
                    ExceptionDecision::RestoreAndForward
                );
            }
        }
    }

    #[test]
    fn accumulated_dr6_slot_zero_then_one_uses_rip_and_clears_all_b_bits() {
        let addresses = [11, 22, 33, 44];
        let mut policy = ExceptionPolicy::default();
        for (slot, dr6) in [(0, 0xFFFF0FF1), (1, 0xFFFF0FF3)] {
            let exception = RawException {
                tid: 9,
                code: 0x80000004,
                first_chance: 1,
                exception_address: addresses[slot],
            };
            let snapshot = ProbeSnapshot {
                armed: true,
                registers: DebugRegisters {
                    dr6,
                    ..DebugRegisters::default().installed(addresses, 0x55)
                },
                rip: addresses[slot],
                flags: 0x202,
            };
            let decision = policy.decide(exception, Some(snapshot), addresses, 0x55);
            assert_eq!(
                decision,
                ExceptionDecision::Probe(probe_source(slot, dr6 & 0xF))
            );
            assert!(decision.handled());
            assert!(!decision.stop_observing());
            assert_eq!(
                decision.resume_debug_flags(dr6, 0x202),
                Some((0xFFFF0FF0, 0x10202))
            );
        }
        assert_eq!(policy.unknown_first_chance_count, 0);
    }

    #[test]
    fn uncertain_ownership_restores_and_forwards_without_editing_exception_sources() {
        let addresses = [11, 22, 33, 44];
        let exception = RawException {
            tid: 9,
            code: 0x80000004,
            first_chance: 1,
            exception_address: 22,
        };
        let snapshot = ProbeSnapshot {
            armed: true,
            registers: DebugRegisters {
                address: [11, 99, 33, 44],
                dr6: 3,
                dr7: 0x55,
            },
            rip: 22,
            flags: 0x202,
        };
        let mut policy = ExceptionPolicy::default();
        {
            let decision = policy.decide(exception, Some(snapshot), addresses, 0x55);
            assert_eq!(decision, ExceptionDecision::RestoreAndForward);
            assert!(!decision.handled());
            assert!(decision.stop_observing());
            assert_eq!(decision.resume_debug_flags(0xE003, 0x202), None);
        }
        let valid_snapshot = ProbeSnapshot {
            registers: DebugRegisters {
                address: addresses,
                ..snapshot.registers
            },
            ..snapshot
        };
        for unavailable in [
            None,
            Some(ProbeSnapshot {
                armed: false,
                ..valid_snapshot
            }),
        ] {
            assert_eq!(
                policy.decide(exception, unavailable, addresses, 0x55),
                ExceptionDecision::RestoreAndForward
            );
        }
        // An ExceptionAddress mismatch is uncertainty, never ownership proof.
        assert_eq!(
            policy.decide(
                RawException {
                    exception_address: 99,
                    ..exception
                },
                Some(valid_snapshot),
                addresses,
                0x55
            ),
            ExceptionDecision::RestoreAndForward
        );
        let bad_dr7 = ProbeSnapshot {
            registers: DebugRegisters {
                dr7: 0x10055,
                ..valid_snapshot.registers
            },
            ..valid_snapshot
        };
        let decision = policy.decide(exception, Some(bad_dr7), addresses, 0x55);
        assert_eq!(decision, ExceptionDecision::RestoreAndForward);
        assert!(!decision.handled());
        assert_eq!(
            decision.resume_debug_flags(bad_dr7.registers.dr6, bad_dr7.flags),
            None
        );
        for owns_probes in [true, false] {
            let foreign = ProbeSnapshot {
                rip: 99,
                ..snapshot
            };
            let decision = policy.decide(
                RawException {
                    exception_address: 99,
                    ..exception
                },
                Some(foreign),
                addresses,
                if owns_probes { 0x55 } else { 0 },
            );
            assert_eq!(
                decision,
                if owns_probes {
                    ExceptionDecision::RestoreAndForward
                } else {
                    ExceptionDecision::ForwardFirstChance { record: true }
                }
            );
            assert!(!decision.handled());
            assert_eq!(decision.stop_observing(), owns_probes);
            assert_eq!(decision.resume_debug_flags(3, 0x202), None);
        }
        // A matching static RVA alone does not authorize a probe before any
        // hardware registers have actually been installed by this session.
        assert_eq!(
            policy.decide(exception, Some(snapshot), addresses, 0),
            ExceptionDecision::ForwardFirstChance { record: true }
        );
    }

    #[test]
    fn second_chance_never_handles_matching_or_uncertain_owned_single_step() {
        let addresses = [11, 22, 33, 44];
        let mut policy = ExceptionPolicy::default();
        for enabled in [0x45, 0x55] {
            for slot in 0..4 {
                for app_bits in [0, 0x2000, 0x4000, 0x8000, 0xE000] {
                    for flags in [0x202, 0x302] {
                        let snapshot = ProbeSnapshot {
                            armed: true,
                            registers: DebugRegisters {
                                dr6: (1 << slot) | app_bits,
                                ..DebugRegisters::default().installed(addresses, enabled)
                            },
                            rip: addresses[slot],
                            flags,
                        };
                        for context in [
                            Some(snapshot),
                            None,
                            Some(ProbeSnapshot {
                                registers: DebugRegisters {
                                    dr7: 0,
                                    ..snapshot.registers
                                },
                                ..snapshot
                            }),
                        ] {
                            let decision = policy.decide(
                                RawException {
                                    tid: 9,
                                    code: 0x80000004,
                                    first_chance: 0,
                                    exception_address: addresses[slot],
                                },
                                context,
                                addresses,
                                enabled,
                            );
                            assert_eq!(decision, ExceptionDecision::ForwardSecondChance);
                            assert!(!decision.handled());
                            assert!(decision.stop_observing());
                            assert_eq!(
                                decision.resume_debug_flags(snapshot.registers.dr6, flags),
                                None
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(policy.second_chance_count, 240);
        assert_eq!(policy.unknown_first_chance_count, 0);
    }

    #[test]
    fn composite_removes_only_enabled_owned_b_bits_and_keeps_disabled_source() {
        let addresses = [11, 22, 33, 44];
        let snapshot = ProbeSnapshot {
            armed: true,
            registers: DebugRegisters {
                dr6: 0xFFFFEFFD,
                ..DebugRegisters::default().installed(addresses, 0x45)
            },
            rip: 11,
            flags: 0x302,
        };
        let decision = ExceptionPolicy::default().decide(
            RawException {
                tid: 9,
                code: 0x80000004,
                first_chance: 1,
                exception_address: 11,
            },
            Some(snapshot),
            addresses,
            0x45,
        );
        assert_eq!(
            decision,
            ExceptionDecision::ForwardComposite(ProbeSource {
                slot: 0,
                owned_b_bits: 9,
                app_dr6: 0xE004,
                app_tf: true,
            })
        );
        assert!(!decision.handled());
        assert_eq!(
            decision.resume_debug_flags(0xFFFFEFFD, 0x302),
            Some((0xFFFFEFF4, 0x10302))
        );
    }

    #[test]
    fn resume_readback_rejects_each_mismatch_and_unreadable_context() {
        let requested = ProbeSnapshot {
            armed: true,
            registers: DebugRegisters {
                address: [11, 22, 33, 44],
                dr6: 0xE000,
                dr7: 0x55,
            },
            rip: 33,
            flags: 0x10302,
        };
        assert_eq!(validate_resume_readback(requested, Ok(requested)), Ok(()));
        assert_eq!(
            validate_resume_readback(
                requested,
                Ok(ProbeSnapshot {
                    flags: requested.flags & !2,
                    ..requested
                })
            ),
            Ok(())
        );
        // Only architectural fixed/reserved bits may vary; actual RF is required.
        assert_eq!(
            validate_resume_readback(
                requested,
                Ok(ProbeSnapshot {
                    registers: DebugRegisters {
                        dr6: 0xFFFFEFF0,
                        dr7: 0x455,
                        ..requested.registers
                    },
                    ..requested
                })
            ),
            Ok(())
        );
        for (actual, expected_error) in [
            (Err("GetThreadContext denied".into()), "readback failed"),
            (
                Ok(ProbeSnapshot {
                    registers: DebugRegisters {
                        dr6: 0xE004,
                        ..requested.registers
                    },
                    ..requested
                }),
                "DR6",
            ),
            (
                Ok(ProbeSnapshot {
                    registers: DebugRegisters {
                        dr6: 0xA000,
                        ..requested.registers
                    },
                    ..requested
                }),
                "DR6",
            ),
            (
                Ok(ProbeSnapshot {
                    registers: DebugRegisters {
                        dr7: 0x45,
                        ..requested.registers
                    },
                    ..requested
                }),
                "DR0..DR3/DR7",
            ),
            (
                Ok(ProbeSnapshot {
                    registers: DebugRegisters {
                        address: [11, 22, 99, 44],
                        ..requested.registers
                    },
                    ..requested
                }),
                "DR0..DR3/DR7",
            ),
            (
                Ok(ProbeSnapshot {
                    rip: 34,
                    ..requested
                }),
                "RIP",
            ),
            (
                Ok(ProbeSnapshot {
                    flags: 0x302,
                    ..requested
                }),
                "RF",
            ),
            (
                Ok(ProbeSnapshot {
                    flags: 0x10202,
                    ..requested
                }),
                "application EFlags",
            ),
        ] {
            assert!(validate_resume_readback(requested, actual)
                .unwrap_err()
                .contains(expected_error));
        }
    }

    #[test]
    fn trace_capacity_prioritizes_traps_and_records_omission_causes() {
        use serde_json::json;
        let mut records = Vec::new();
        let mut omitted = std::collections::BTreeMap::new();
        for tid in 0..150 {
            retain_trace(
                &mut records,
                &mut omitted,
                json!({"event":"probe_install","tid":tid}),
            );
        }
        for ordinal in 0..90 {
            retain_trace(
                &mut records,
                &mut omitted,
                json!({"event":if ordinal % 2 == 0 { "probe_resume" } else { "debug_continue" },"ordinal":ordinal}),
            );
        }
        for event in ["exit", "error"] {
            retain_trace(&mut records, &mut omitted, json!({"event":event}));
        }
        assert_eq!(records.len(), 128);
        assert_eq!(omitted["probe_install/capacity"], 22);
        assert_eq!(omitted["probe_install/replaced_by_priority"], 92);
        assert_eq!(
            records
                .iter()
                .filter(|r| r["event"] == "probe_resume")
                .count(),
            45
        );
        assert_eq!(
            records
                .iter()
                .filter(|r| r["event"] == "debug_continue")
                .count(),
            45
        );
        assert_eq!(
            records
                .iter()
                .filter(|r| r["event"] == "exit" || r["event"] == "error")
                .count(),
            2
        );
        records = (0..128)
            .map(|_| json!({"event":"debug_continue"}))
            .collect();
        retain_trace(&mut records, &mut omitted, json!({"event":"probe_resume"}));
        assert_eq!(records.len(), 128);
        assert_eq!(omitted["probe_resume/capacity"], 1);
    }

    #[test]
    fn unknown_first_chance_records_are_bounded_and_second_chance_ends_observation() {
        let mut policy = ExceptionPolicy::default();
        let addresses = [11, 22, 33, 44];
        for ordinal in 0..1000 {
            let code = if ordinal % 2 == 0 {
                0xE06D7363
            } else {
                0x406D1388
            };
            let exception = RawException {
                tid: 9,
                code,
                first_chance: 1,
                exception_address: 11,
            };
            let decision = policy.decide(exception, None, addresses, 0x55);
            assert_eq!(
                decision,
                ExceptionDecision::ForwardFirstChance {
                    record: ordinal < 16
                }
            );
            assert!(!decision.handled());
            assert!(!decision.stop_observing());
        }
        assert_eq!(policy.unknown_first_chance_count, 1000);
        assert_eq!(
            policy.unknown_first_chance_records,
            FIRST_CHANCE_RECORD_LIMIT
        );
        let decision = policy.decide(
            RawException {
                tid: 9,
                code: 0xC0000005,
                first_chance: 0,
                exception_address: 11,
            },
            None,
            addresses,
            0x55,
        );
        assert_eq!(decision, ExceptionDecision::ForwardSecondChance);
        assert!(!decision.handled());
        assert!(decision.stop_observing());
        assert_eq!(policy.second_chance_count, 1);
    }

    #[test]
    fn raw_read_error_keeps_observing_and_is_not_a_genuine_access_exception() {
        let addresses = [11, 22, 33, 44];
        let mut policy = ExceptionPolicy::default();
        let mut observation = Observation::default();
        observation.setter(0x10000, 0x11630, 9, 100).unwrap();
        observation.returned(0x10000, 9, 100);
        let snapshot = ProbeSnapshot {
            armed: true,
            registers: DebugRegisters {
                dr6: 4,
                ..DebugRegisters::default().installed(addresses, 0x55)
            },
            rip: 33,
            flags: 0x202,
        };
        for read_result in [Err("mock null or unreadable RAX field".into()), Ok(123i32)] {
            let decision = policy.decide(
                RawException {
                    tid: 9,
                    code: 0x80000004,
                    first_chance: 1,
                    exception_address: 33,
                },
                Some(snapshot),
                addresses,
                0x55,
            );
            assert_eq!(decision, ExceptionDecision::Probe(probe_source(2, 4)));
            assert!(decision.handled());
            assert!(!decision.stop_observing());
            assert!(observation.getter());
            let raw = raw_read(read_result);
            if observation.getters_recorded == 1 {
                assert!(raw["read_error"].is_string());
            } else {
                assert_eq!(raw, serde_json::json!(123));
            }
        }
        assert_eq!(observation.getters_recorded, 2);
        assert_eq!(policy.unknown_first_chance_count, 0);
        let decision = policy.decide(
            RawException {
                tid: 9,
                code: 0xC0000005,
                first_chance: 1,
                exception_address: 33,
            },
            Some(snapshot),
            addresses,
            0x55,
        );
        assert_eq!(
            decision,
            ExceptionDecision::ForwardFirstChance { record: true }
        );
        assert!(!decision.handled()); // never convert a target AV into a read error
    }

    #[test]
    fn attach_break_accepts_verified_breakin_thread_different_from_process_event_thread_once() {
        let image = verified_fixture();
        let main_tid = 51932;
        let breakin_tid = 45400;
        let mut policy = ExceptionPolicy::default();
        policy.attach.bind(image.clone());
        assert!(!policy
            .attach
            .register(main_tid, None, Ok(image.breakin_address()), true)
            .unwrap());
        assert!(policy
            .attach
            .register(breakin_tid, None, Ok(image.breakin_address()), false)
            .unwrap());
        let event = breakpoint(breakin_tid, image.breakpoint_address());
        assert_eq!(
            policy.decide(event, None, [11, 22, 33, 44], 0),
            ExceptionDecision::AttachBreak
        );
        assert_eq!(
            policy.decide(event, None, [11, 22, 33, 44], 0),
            ExceptionDecision::ForwardFirstChance { record: true }
        );
    }

    #[test]
    fn attach_break_rejects_same_address_on_app_threads_other_start_or_unverified_image() {
        let image = verified_fixture();
        for (event_start, os_start, preexisting, bind) in [
            (None, Ok(image.breakin_address() + 1), false, true),
            (
                Some(image.breakin_address() + 1),
                Ok(image.breakin_address()),
                false,
                true,
            ),
            (None, Ok(image.breakin_address()), true, true),
            (None, Ok(image.breakin_address()), false, false),
            (None, Err("OS query unavailable".into()), false, true),
        ] {
            let mut policy = ExceptionPolicy::default();
            if bind {
                policy.attach.bind(image.clone());
            }
            assert!(!policy
                .attach
                .register(7, event_start, os_start, preexisting)
                .unwrap_or(false));
            let decision = policy.decide(
                breakpoint(7, image.breakpoint_address()),
                None,
                [11, 22, 33, 44],
                0,
            );
            assert_eq!(
                decision,
                ExceptionDecision::ForwardFirstChance { record: true }
            );
            assert!(!decision.handled());
        }
    }

    #[test]
    fn attach_break_os_query_identifies_null_event_start_and_rejects_second_chance() {
        let image = verified_fixture();
        let mut policy = ExceptionPolicy::default();
        policy.attach.bind(image.clone());
        assert!(policy
            .attach
            .register(7, None, Ok(image.breakin_address()), false)
            .unwrap());
        let second = RawException {
            first_chance: 0,
            ..breakpoint(7, image.breakpoint_address())
        };
        assert_eq!(
            policy.decide(second, None, [11, 22, 33, 44], 0),
            ExceptionDecision::ForwardSecondChance
        );
        assert!(!policy
            .attach
            .consume(breakpoint(8, image.breakpoint_address())));
        assert!(!policy
            .attach
            .consume(breakpoint(7, image.breakpoint_address() + 1)));
        assert!(policy
            .attach
            .consume(breakpoint(7, image.breakpoint_address())));
        policy.attach.retire(7);
        assert!(!policy
            .attach
            .consume(breakpoint(7, image.breakpoint_address())));
    }

    #[test]
    fn attach_pe_direct_exports_and_short_remote_reads_follow_aslr_and_section_mapping() {
        let file = pe_fixture();
        let image = PeImage::parse(&file).unwrap();
        for base in [0x180000000, 0x7FFF12340000] {
            let reader = FixtureReader::new(&file, base);
            let verified = image.verify_loaded(&reader, base, 0x5000).unwrap();
            assert_eq!(verified.breakpoint_address(), base + 0x1010);
            assert_eq!(verified.breakin_address(), base + 0x1030);
            assert_eq!(
                image.export("DbgBreakPoint").unwrap().prefix[..2],
                [0xCC, 0xC3]
            );
            assert!(reader.calls.borrow().len() < 128);
            assert!(reader.calls.borrow().iter().all(|(_, size)| *size <= 8));
        }
    }

    #[test]
    fn attach_pe_rejects_file_header_export_forwarder_ordinal_and_prefix_mismatch() {
        for (offset, replacement) in [
            (0, vec![0, 0]),
            (0x84, vec![0x4C, 0x01]),
            (0x98, vec![0x0B, 0x01]),
            (0x640, 0x3100u32.to_le_bytes().to_vec()),
            (0x680, 3u16.to_le_bytes().to_vec()),
            (0x210, vec![0x90, 0xC3]),
            (0x98 + 56, 1u32.to_le_bytes().to_vec()),
        ] {
            let mut bytes = pe_fixture();
            bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
            assert!(
                PeImage::parse(&bytes).is_err(),
                "accepted mismatch at {offset:#x}"
            );
        }
        let file = pe_fixture();
        for length in [0, 64, 0x90, 0x1A0, 0x601, 0xA00 - 1] {
            assert!(PeImage::parse(&file[..length]).is_err());
        }
    }

    #[test]
    fn attach_pe_rejects_loaded_header_export_bytes_and_wrong_image_base() {
        let file = pe_fixture();
        let image = PeImage::parse(&file).unwrap();
        for rva in [
            0x80,
            0x98 + 56,
            0x3040,
            0x3060,
            0x3080,
            0x30A0,
            0x1010,
            0x1030,
            0x1050,
        ] {
            let mut reader = FixtureReader::new(&file, 0x7FFF00000000);
            reader.bytes[rva] ^= 1;
            assert!(
                image.verify_loaded(&reader, reader.base, 0x5000).is_err(),
                "accepted changed loaded field {rva:#x}"
            );
        }
        let reader = FixtureReader::new(&file, 0x7FFF00000000);
        assert!(image
            .verify_loaded(&reader, reader.base + 1, 0x5000)
            .is_err());
        assert!(image.verify_loaded(&reader, reader.base, 0x6000).is_err());
        assert!(image.verify_loaded(&reader, u64::MAX - 1, 0x5000).is_err());
    }

    #[test]
    fn attach_registration_then_error_keeps_cleanup_finite_and_never_accepts_ambiguous_thread() {
        let image = verified_fixture();
        let mut policy = ExceptionPolicy::default();
        policy.attach.bind(image.clone());
        assert!(policy
            .attach
            .register(7, None, Ok(image.breakin_address()), false)
            .unwrap());
        assert!(policy
            .attach
            .register(8, None, Ok(image.breakin_address()), false)
            .is_err());
        let decision = policy.decide(
            breakpoint(7, image.breakpoint_address()),
            None,
            [11, 22, 33, 44],
            0,
        );
        assert_eq!(
            decision,
            ExceptionDecision::ForwardFirstChance { record: true }
        );
        let mut backend = MockCleanup::default();
        let report = cleanup(&mut backend);
        assert_eq!(
            backend.calls,
            [
                "freeze",
                "restore:10",
                "restore:20",
                "restore:30",
                "continue",
                "drain",
                "detach",
                "resume"
            ]
        );
        assert!(report.complete());
    }

    #[test]
    fn attach_no_ready_deadline_or_exit_never_reports_a_successful_diagnostic() {
        for reason in ["deadline", "cancelled", "process_exited"] {
            assert!(require_ready(Ok(reason), false).is_err());
        }
        assert_eq!(require_ready(Ok("deadline"), true).unwrap(), "deadline");
        assert_eq!(
            require_ready(Err("setup error".into()), false).unwrap_err(),
            "setup error"
        );
    }

    #[test]
    fn first_setter_pins_receiver_thread_and_stack_without_retargeting() {
        let mut state = Observation::default();
        assert!(state.setter(0x10000, 0x11630, 9, 100).unwrap());
        assert!(!state.setter(0x20000, 0x21630, 8, 200).unwrap());
        assert!(!state.returned(0x20000, 9, 100));
        assert!(!state.returned(0x10000, 8, 100));
        assert!(!state.returned(0x10000, 9, 200));
        assert!(!state.candidate(0x21630, 9));
        assert!(!state.candidate(0x11630, 8));
        assert!(state.candidate(0x11630, 9));
        assert!(state.returned(0x10000, 9, 100));
        assert!(!state.returned(0x10000, 9, 100));
        assert!(!state.candidate(0x11630, 9));
        assert_eq!(state.trigger.unwrap().cache_city, 0x10000);
        assert!(Observation::default().setter(u64::MAX, 0, 1, 1).is_err());
        assert!(Observation::default().setter(0, 0x1630, 1, 1).is_err());
    }

    #[test]
    fn repeated_setter_and_candidate_volume_end_the_first_call_window() {
        let mut state = Observation::default();
        state.setter(0x10000, 0x11630, 9, 100).unwrap();
        for _ in 0..CANDIDATE_LIMIT {
            assert!(state.candidate(0x11630, 9));
        }
        assert!(!state.candidate(0x11630, 9));
        assert_eq!(state.candidates_recorded, CANDIDATE_LIMIT);
        assert!(!state.setter(0x20000, 0x21630, 9, 110).unwrap());
        assert!(!state.another_setter_for_trigger_cache);
        assert!(!state.setter(0x10000, 0x11630, 9, 120).unwrap());
        assert!(state.another_setter_for_trigger_cache);
        assert_eq!(state.trigger.unwrap().rsp, 100);
    }

    #[test]
    fn natural_getters_do_not_consume_setter_log_budget() {
        let mut state = Observation::default();
        for _ in 0..10000 {
            assert!(!state.getter());
        }
        assert_eq!(state.getters_recorded, 0);
        state.setter(0x10000, 0x11630, 9, 100).unwrap();
        assert!(!state.getter()); // before the first wrapper return
        state.returned(0x10000, 9, 100);
        for _ in 0..GETTER_LIMIT {
            assert!(state.getter());
        }
        assert!(!state.getter());
        assert_eq!(state.getters_recorded, GETTER_LIMIT);
        assert_eq!(state.getters_before_return, 10001);
    }

    #[derive(Default)]
    struct MockCleanup {
        calls: Vec<String>,
        fail_freeze: bool,
        fail_restore: bool,
        fail_continue: bool,
        fail_drain: bool,
        fail_detach: bool,
        fail_resume: bool,
        exited: bool,
    }

    impl CleanupBackend for MockCleanup {
        fn freeze(&mut self) -> Vec<String> {
            self.calls.push("freeze".into());
            if self.fail_freeze {
                vec!["injected freeze failure".into()]
            } else {
                Vec::new()
            }
        }
        fn armed_threads(&self) -> Vec<u32> {
            vec![10, 20, 30]
        }
        fn restore(&mut self, tid: u32) -> Result<RestoreOutcome> {
            self.calls.push(format!("restore:{tid}"));
            if self.fail_restore && tid == 10 {
                Err("injected context failure".into())
            } else if tid == 20 {
                Ok(RestoreOutcome::ThreadExited)
            } else {
                Ok(RestoreOutcome::Restored)
            }
        }
        fn continue_pending(&mut self) -> Result<()> {
            self.calls.push("continue".into());
            if self.fail_continue {
                Err("injected continue failure".into())
            } else {
                Ok(())
            }
        }
        fn detach(&mut self) -> Result<bool> {
            self.calls.push("detach".into());
            if self.fail_detach {
                Err("injected detach failure".into())
            } else {
                Ok(!self.exited)
            }
        }
        fn drain_events(&mut self) -> Result<()> {
            self.calls.push("drain".into());
            if self.fail_drain {
                Err("injected drain failure".into())
            } else {
                Ok(())
            }
        }
        fn resume(&mut self) -> Vec<String> {
            self.calls.push("resume".into());
            if self.fail_resume {
                vec!["injected resume failure".into()]
            } else {
                Vec::new()
            }
        }
        fn process_exited(&self) -> bool {
            self.exited
        }
    }

    #[test]
    fn cleanup_restores_all_before_continue_and_detach_even_on_failure() {
        for failures in 0..64 {
            let mut mock = MockCleanup {
                fail_restore: failures & 1 != 0,
                fail_continue: failures & 2 != 0,
                fail_detach: failures & 4 != 0,
                fail_freeze: failures & 8 != 0,
                fail_drain: failures & 16 != 0,
                fail_resume: failures & 32 != 0,
                ..Default::default()
            };
            let report = cleanup(&mut mock);
            assert_eq!(
                mock.calls,
                [
                    "freeze",
                    "restore:10",
                    "restore:20",
                    "restore:30",
                    "continue",
                    "drain",
                    "detach",
                    "resume"
                ]
            );
            assert_eq!(report.complete(), failures == 0);
            assert_eq!(report.errors.len(), (failures as u32).count_ones() as usize);
            assert_eq!(report.threads_exited, 1);
        }
        let mut mock = MockCleanup {
            exited: true,
            ..Default::default()
        };
        let report = cleanup(&mut mock);
        assert!(report.complete());
        assert!(!report.debug_active_process_stop_ok);
        assert!(report.process_exited);
    }

    #[test]
    fn cli_requires_finite_duration_and_absolute_new_jsonl_name() {
        let mut args = Args {
            pid: 123,
            created: 456,
            output: std::env::current_dir().unwrap().join("probe.jsonl"),
            seconds: 90,
        };
        assert!(args.validate().is_ok());
        assert!(args.ready_path().ends_with("probe.jsonl.ready"));
        args.seconds = 0;
        assert!(args.validate().is_err());
        args.seconds = 91;
        assert!(args.validate().is_err());
        args.seconds = 90;
        args.output = PathBuf::from("relative.jsonl");
        assert!(args.validate().is_err());
    }
}
