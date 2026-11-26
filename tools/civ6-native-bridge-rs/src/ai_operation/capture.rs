//! Capture-only callback body candidate. Reuses mode-8's raw register ABI and
//! the existing guarded Governor reader. No detour installer, no native call,
//! no VM entry, no game-memory writes. A validated runtime gateway must call
//! these methods at the listed natural boundaries, on its own current thread.
//! Merely supplying this Rust struct is NOT proof that a gateway was installed.
use super::protocol::Epoch;
use civ6_native_bridge_rs::{
    governor_payment::{read_current, CurrentMemory, PaymentDefinition, PromotionSnapshot},
    PromotionObservation,
};
use sha2::{Digest, Sha256};
use windows_sys::Win32::System::{
    Diagnostics::Debug::ReadProcessMemory,
    Threading::{GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, GetProcessTimes},
};

pub const CALLBACK_ENTRY: u64 = 0x4a7ad0;
pub const AI_CHECK_RETURN: u64 = 0x4a7d6c;
pub const OFFICIAL_CHECK_RETURN: u64 = 0x30ec14;
pub const APPROVAL_RETURN: u64 = 0x30ec29;
pub const EXECUTE_CALL: u64 = 0x4a7f18;

/// Actual current-process read primitive for the existing read_current API.
/// This candidate never opens a game process or uses a retained foreign handle.
pub struct CurrentProcessMemory;
impl CurrentMemory for CurrentProcessMemory {
    fn copy<const N: usize>(&self, address: u64) -> Option<[u8; N]> {
        if address == 0 || address.checked_add(N as u64).is_none() {
            return None;
        }
        let mut value = [0; N];
        let mut copied = 0;
        let ok = unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                address as *const _,
                value.as_mut_ptr().cast(),
                N,
                &mut copied,
            )
        };
        (ok != 0 && copied == N).then_some(value)
    }
}

fn u64_field(m: &impl CurrentMemory, address: u64, offset: u64) -> Option<u64> {
    Some(u64::from_le_bytes(m.copy(address.checked_add(offset)?)?))
}
fn u32_field(m: &impl CurrentMemory, address: u64, offset: u64) -> Option<u32> {
    Some(u32::from_le_bytes(m.copy(address.checked_add(offset)?)?))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Context,
    Order,
    Read,
    Owner,
    Arguments,
    Spent,
    Payment,
    Filter,
    Missing,
}

#[derive(Clone, Copy)]
pub struct Context {
    pub epoch: Epoch,
    pub thread: u32,
    pub module_base: u64,
}
impl Context {
    /// Verifies real PID creation and thread at each callback, not caller labels.
    /// Generation must still come from the synchronized runtime's load boundary.
    pub fn matches_current_process(&self) -> bool {
        use windows_sys::Win32::Foundation::FILETIME;
        let mut created = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = created;
        let mut kernel = created;
        let mut user = created;
        let ok = unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        };
        ok != 0
            && self.epoch.pid == unsafe { GetCurrentProcessId() }
            && self.thread == unsafe { GetCurrentThreadId() }
            && self.epoch.created
                == (u64::from(created.dwHighDateTime) << 32 | u64::from(created.dwLowDateTime))
            && self.epoch.generation != 0
            && self.module_base != 0
    }
}

/// A receipt includes raw mode-8-compatible rows for all three native calls.
/// It is a calibration result, never a runtime payment/admission permit.
#[derive(Debug)]
pub struct Receipt {
    pub epoch: Epoch,
    pub thread: u32,
    pub owner: u32,
    pub governor_index: u32,
    pub promotion_index: u32,
    pub ai_check: PromotionObservation,
    pub official_check: PromotionObservation,
    pub approval: PromotionObservation,
    pub owner_filter: PromotionObservation,
    pub spent_before_call: SpentSample,
    pub spent_after_call: SpentSample,
    pub spent_before: u32,
    pub spent_after: u32,
    pub own_before: PromotionSnapshot,
    pub own_after: PromotionSnapshot,
    pub payment_authority: bool,
    pub returns: [ReturnObservation; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct ReturnObservation {
    pub raw: PromotionObservation,
    pub rip: u64,
    pub rax: u64,
}

impl Receipt {
    /// Finite artifact for existing acceptance tooling to inspect. Does not
    /// create a gameplay permit or claim all three payments were observed.
    pub fn artifact(&self) -> serde_json::Value {
        use serde_json::json;
        let raw = |r: PromotionObservation| {
            json!({"sequence":r.sequence,"thread":r.thread_id,
            "rcx":r.rcx,"rdx":r.rdx,"r8":r.r8,"r9":r.r9,"return_address":r.return_address})
        };
        let returned =
            |r: ReturnObservation| json!({"raw":raw(r.raw),"rip":r.rip,"original_rax":r.rax});
        let spent = |s: SpentSample| json!({"entry":raw(s.entry),"returned":returned(s.returned)});
        json!({"schema":"todo880-ai-one-title-capture-v1", "pid":self.epoch.pid,
            "created_filetime":self.epoch.created,"generation":self.epoch.generation,
            "thread":self.thread,"owner":self.owner,"governor_index":self.governor_index,
            "promotion_index":self.promotion_index,"ai_check":raw(self.ai_check),
            "official_check":raw(self.official_check),"approval":raw(self.approval),
            "owner_filter":raw(self.owner_filter),"spent_before_call":spent(self.spent_before_call),
            "spent_after_call":spent(self.spent_after_call),"original_returns":self.returns.map(returned),
            "spent_before":self.spent_before,"spent_after":self.spent_after,
            "promotion_count":self.own_before.count,"own_before_words":self.own_before.owned.to_vec(),
            "own_after_words":self.own_after.owned.to_vec(),"payment_authority":false,
            "full_cost_completion":false,"ai_acceptance":false,
            "provenance_required":"natural runtime observer installation and synchronized definition/ledger binding remain external prerequisites"})
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SpentSample {
    pub entry: PromotionObservation,
    pub returned: ReturnObservation,
}

/// Initialized only inside the natural AI callback entry, not a property read.
/// Dropped/changed/unknown samples abandon this instance; no retry/send exists.
pub struct Capture {
    context: Context,
    player: u64,
    owner: u32,
    definition: PaymentDefinition,
    promotion: u32,
    governors: u64,
    governor: u64,
    before: Option<PromotionSnapshot>,
    spent_before: Option<u32>,
    ai: Option<PromotionObservation>,
    official: Option<PromotionObservation>,
    approval: Option<PromotionObservation>,
    ai_allowed: bool,
    official_allowed: bool,
    filter_allowed: bool,
    execute_seen: bool,
    last_sequence: u32,
    filter: Option<PromotionObservation>,
    approval_returned: bool,
    poisoned: bool,
    expected_owner_after: Option<(u32, [u8; 32])>,
    spent_before_call: Option<SpentSample>,
    spent_entry: Option<PromotionObservation>,
    returns: [Option<ReturnObservation>; 4],
}

impl Capture {
    pub fn begin(
        context: Context,
        rip: u64,
        raw: PromotionObservation,
        def: PaymentDefinition,
        promotion: u32,
        memory: &impl CurrentMemory,
    ) -> Result<Self, Error> {
        if !context.matches_current_process()
            || rip
                != context
                    .module_base
                    .checked_add(CALLBACK_ENTRY)
                    .ok_or(Error::Context)?
            || raw.thread_id != context.thread
            || raw.return_address.checked_sub(context.module_base) != Some(0x401537)
            || raw.rdx as u32 != 2
            || raw.sequence == 0
        {
            return Err(Error::Context);
        }
        let player = u64_field(memory, raw.rcx, 0x98).ok_or(Error::Read)?;
        let owner = u32_field(memory, player, 0xd8).ok_or(Error::Read)?;
        if owner >= 64 || !def.titles.contains(&Some(promotion)) {
            return Err(Error::Owner);
        }
        Ok(Self {
            context,
            player,
            owner,
            definition: def,
            promotion,
            governors: 0,
            governor: 0,
            before: None,
            spent_before: None,
            ai: None,
            official: None,
            approval: None,
            ai_allowed: false,
            official_allowed: false,
            filter_allowed: false,
            execute_seen: false,
            last_sequence: raw.sequence,
            filter: None,
            approval_returned: false,
            poisoned: false,
            expected_owner_after: None,
            spent_before_call: None,
            spent_entry: None,
            returns: [None; 4],
        })
    }

    fn event(&mut self, context: Context, raw: &PromotionObservation) -> Result<(), Error> {
        if self.poisoned {
            return Err(Error::Missing);
        }
        self.poisoned = true;
        if !context.matches_current_process()
            || context.epoch != self.context.epoch
            || context.module_base != self.context.module_base
            || context.thread != self.context.thread
            || raw.thread_id != self.context.thread
        {
            return Err(Error::Context);
        }
        if raw.sequence <= self.last_sequence {
            return Err(Error::Order);
        }
        self.last_sequence = raw.sequence;
        Ok(())
    }

    /// Raw arguments at native callee entry, preserving original return address.
    /// AI/checker RDX is a pointer; approval EDX is an Index. R9 stays raw.
    pub fn callee(
        &mut self,
        context: Context,
        raw: PromotionObservation,
        memory: &impl CurrentMemory,
    ) -> Result<(), Error> {
        self.event(context, &raw)?;
        let site = raw
            .return_address
            .checked_sub(context.module_base)
            .ok_or(Error::Arguments)?;
        if raw.r8 as u32 != self.promotion {
            return Err(Error::Arguments);
        }
        let player = u64_field(memory, raw.rcx, 0x70).ok_or(Error::Read)?;
        if player != self.player || u32_field(memory, player, 0xd8) != Some(self.owner) {
            return Err(Error::Owner);
        }
        match site {
            AI_CHECK_RETURN if self.ai.is_none() => {
                let own = read_current(memory, raw.rcx, raw.rdx).map_err(|_| Error::Read)?;
                let paid = own
                    .diagnostic_paid(self.owner, self.definition)
                    .map_err(|_| Error::Payment)?;
                if paid == 0
                    || paid >= self.definition.cost
                    || self.definition.titles[paid as usize - 1] != Some(self.promotion)
                {
                    return Err(Error::Payment);
                }
                self.governors = raw.rcx;
                self.governor = raw.rdx;
                self.before = Some(own);
                self.expected_owner_after =
                    Some(aggregate(memory, raw.rcx, Some((raw.rdx, self.promotion)))?);
                self.ai = Some(raw);
            }
            OFFICIAL_CHECK_RETURN if self.execute_seen && self.official.is_none() => {
                if raw.rcx != self.governors
                    || raw.rdx != self.governor
                    || Some(read_current(memory, raw.rcx, raw.rdx).map_err(|_| Error::Read)?)
                        != self.before
                {
                    return Err(Error::Payment);
                }
                self.official = Some(raw);
            }
            APPROVAL_RETURN if self.official_allowed && self.approval.is_none() => {
                if raw.rcx != self.governors
                    || raw.rdx as u32 != self.definition.governor_index
                    || Some(read_current(memory, raw.rcx, self.governor).map_err(|_| Error::Read)?)
                        != self.before
                {
                    return Err(Error::Arguments);
                }
                self.approval = Some(raw);
            }
            _ => return Err(Error::Order),
        }
        self.poisoned = false;
        Ok(())
    }

    /// Natural filter call 4A7EF0. Capture the actual four register values;
    /// R8B must remain false. The result is separately observed at 4A7EF5.
    pub fn filter_entry(
        &mut self,
        context: Context,
        raw: PromotionObservation,
        memory: &impl CurrentMemory,
    ) -> Result<(), Error> {
        self.event(context, &raw)?;
        if !self.ai_allowed
            || self.filter.is_some()
            || raw.return_address.checked_sub(context.module_base) != Some(0x4a7ef5)
            || raw.rcx as u32 != self.owner
            || raw.r8 as u8 != 0
            || raw.r9 != 0
            || u32_field(memory, raw.rdx, 0) != Some(0x7d3da99d)
        {
            return Err(Error::Filter);
        }
        self.filter = Some(raw);
        self.poisoned = false;
        Ok(())
    }

    /// Natural return-site record. This method does not call a native function.
    /// Existing entry-only mode 8 cannot supply these return records by itself.
    pub fn returned(
        &mut self,
        context: Context,
        raw: PromotionObservation,
        rip: u64,
        rax: u64,
    ) -> Result<(), Error> {
        self.event(context, &raw)?;
        if rax as u8 != 1 {
            return Err(Error::Filter);
        }
        let slot = match rip
            .checked_sub(context.module_base)
            .ok_or(Error::Arguments)?
        {
            AI_CHECK_RETURN
                if self.ai.is_some() && self.spent_before.is_some() && !self.ai_allowed =>
            {
                self.ai_allowed = true;
                0
            }
            0x4a7ef5 if self.filter.is_some() && !self.filter_allowed => {
                self.filter_allowed = true;
                1
            }
            OFFICIAL_CHECK_RETURN if self.official.is_some() && !self.official_allowed => {
                self.official_allowed = true;
                2
            }
            APPROVAL_RETURN if self.approval.is_some() && !self.approval_returned => {
                self.approval_returned = true;
                3
            }
            _ => return Err(Error::Order),
        };
        self.returns[slot] = Some(ReturnObservation { raw, rip, rax });
        self.poisoned = false;
        Ok(())
    }

    /// Natural callsite 4A7F18: RCX wrapper has owner+18 and operation hash+20.
    pub fn execute_prefix(
        &mut self,
        context: Context,
        raw: PromotionObservation,
        rip: u64,
        memory: &impl CurrentMemory,
    ) -> Result<(), Error> {
        self.event(context, &raw)?;
        if !self.filter_allowed
            || self.execute_seen
            || self.spent_before.is_none()
            || rip.checked_sub(context.module_base) != Some(EXECUTE_CALL)
            || u32_field(memory, raw.rcx, 0x18) != Some(self.owner)
            || u32_field(memory, raw.rcx, 0x20) != Some(0x7d3da99d)
        {
            return Err(Error::Arguments);
        }
        self.execute_seen = true;
        self.poisoned = false;
        Ok(())
    }

    /// Read the owner's complete current Governor list twice with the existing
    /// checked reader. Compare the total to an observed ORIGINAL Spent result;
    /// never manufacture Spent from the persona cost or an operation send.
    pub fn spent_enter(
        &mut self,
        context: Context,
        raw: PromotionObservation,
    ) -> Result<(), Error> {
        self.event(context, &raw)?;
        if self.spent_entry.is_some()
            || raw.rcx != self.governors
            || (!self.approval_returned && (self.ai.is_none() || self.ai_allowed))
            || !matches!(
                raw.return_address.checked_sub(context.module_base),
                Some(0x2ddbd0 | 0x713cfe | 0xba92f | 0x2dd77c | 0x2de117 | 0x2de127 | 0x5dedf7)
            )
        {
            return Err(Error::Order);
        }
        self.spent_entry = Some(raw);
        self.poisoned = false;
        Ok(())
    }

    /// Consumes one original Spent return paired to its entry. A finished
    /// capture is consumed; a failed pairing cannot be reused by the caller.
    pub fn spent_return(
        mut self,
        context: Context,
        returned: ReturnObservation,
        memory: &impl CurrentMemory,
    ) -> Result<Captured, Error> {
        let entry = self.spent_entry.take().ok_or(Error::Missing)?;
        if returned.rip != entry.return_address || returned.raw.sequence <= entry.sequence {
            return Err(Error::Order);
        }
        let sample = SpentSample { entry, returned };
        if self.approval_returned {
            self.finish(context, sample, memory).map(Captured::Receipt)
        } else {
            self.spent_before(context, sample, memory)?;
            Ok(Captured::Continue(self))
        }
    }

    fn spent_before(
        &mut self,
        context: Context,
        sample: SpentSample,
        memory: &impl CurrentMemory,
    ) -> Result<(), Error> {
        let raw = sample.entry;
        let original_spent = sample.returned.rax as u32;
        self.event(context, &sample.returned.raw)?;
        if self.ai.is_none()
            || self.ai_allowed
            || self.execute_seen
            || self.spent_before.is_some()
            || raw.rcx != self.governors
            || raw.return_address.checked_sub(context.module_base) != Some(0x2ddbd0)
            || aggregate(memory, self.governors, None)?.0 != original_spent
        {
            return Err(Error::Spent);
        }
        self.spent_before = Some(original_spent);
        self.spent_before_call = Some(sample);
        self.poisoned = false;
        Ok(())
    }

    /// Called after the ORIGINAL 2DF340 returned AL=1 at 30EC29 and after a
    /// fresh ORIGINAL Spent read. The caller must keep both observations in the
    /// same natural synchronized callback; no function invocation occurs here.
    fn finish(
        mut self,
        context: Context,
        sample: SpentSample,
        memory: &impl CurrentMemory,
    ) -> Result<Receipt, Error> {
        let raw = sample.entry;
        let original_spent = sample.returned.rax as u32;
        self.event(context, &sample.returned.raw)?;
        if !self.approval_returned
            || !self.official_allowed
            || raw.rcx != self.governors
            || !matches!(
                raw.return_address.checked_sub(context.module_base),
                Some(0x2ddbd0 | 0x713cfe | 0xba92f | 0x2dd77c | 0x2de117 | 0x2de127 | 0x5dedf7)
            )
        {
            return Err(Error::Missing);
        }
        let before = self.before.ok_or(Error::Missing)?;
        let spent_before = self.spent_before.ok_or(Error::Missing)?;
        let after = read_current(memory, self.governors, self.governor).map_err(|_| Error::Read)?;
        let mut expected = before;
        expected.owned[self.promotion as usize / 64] |= 1 << (self.promotion % 64);
        if after != expected
            || original_spent != spent_before.checked_add(1).ok_or(Error::Spent)?
            || aggregate(memory, self.governors, None)?
                != self.expected_owner_after.ok_or(Error::Missing)?
        {
            return Err(Error::Payment);
        }
        Ok(Receipt {
            epoch: context.epoch,
            thread: context.thread,
            owner: self.owner,
            governor_index: self.definition.governor_index,
            promotion_index: self.promotion,
            ai_check: self.ai.ok_or(Error::Missing)?,
            official_check: self.official.ok_or(Error::Missing)?,
            approval: self.approval.ok_or(Error::Missing)?,
            owner_filter: self.filter.ok_or(Error::Missing)?,
            spent_before_call: self.spent_before_call.ok_or(Error::Missing)?,
            spent_after_call: sample,
            spent_before,
            spent_after: original_spent,
            own_before: before,
            own_after: after,
            payment_authority: false,
            returns: [
                self.returns[0].ok_or(Error::Missing)?,
                self.returns[1].ok_or(Error::Missing)?,
                self.returns[2].ok_or(Error::Missing)?,
                self.returns[3].ok_or(Error::Missing)?,
            ],
        })
    }
}

#[allow(clippy::large_enum_variant)] // Bounded callback state; avoid heap allocation in the observer.
pub enum Captured {
    Continue(Capture),
    Receipt(Receipt),
}

fn aggregate(
    memory: &impl CurrentMemory,
    governors: u64,
    expected_add: Option<(u64, u32)>,
) -> Result<(u32, [u8; 32]), Error> {
    fn pass(
        memory: &impl CurrentMemory,
        governors: u64,
        expected_add: Option<(u64, u32)>,
    ) -> Result<(u32, [u8; 32]), Error> {
        let mut node = u64_field(memory, governors, 0x58).ok_or(Error::Read)?;
        let mut nodes = [0; 256];
        let mut governors_seen = [0; 256];
        let mut digest = Sha256::new();
        let mut total = 0;
        for i in 0..256 {
            if node == 0 {
                return Ok((total, digest.finalize().into()));
            }
            if nodes[..i].contains(&node) {
                return Err(Error::Read);
            }
            nodes[i] = node;
            let governor = u64_field(memory, node, 0).ok_or(Error::Read)?;
            if governors_seen[..i].contains(&governor) {
                return Err(Error::Read);
            }
            governors_seen[i] = governor;
            let mut snapshot =
                read_current(memory, governors, governor).map_err(|_| Error::Read)?;
            if let Some((target, promotion)) = expected_add {
                if target == governor {
                    snapshot.owned[promotion as usize / 64] |= 1 << (promotion % 64);
                }
            }
            digest.update(node.to_le_bytes());
            digest.update(governor.to_le_bytes());
            digest.update(snapshot.owner.to_le_bytes());
            digest.update(snapshot.governor_index.to_le_bytes());
            digest.update(snapshot.count.to_le_bytes());
            for word in snapshot.owned {
                digest.update(word.to_le_bytes());
                total += word.count_ones();
            }
            node = u64_field(memory, node, 0x10).ok_or(Error::Read)?;
        }
        if node == 0 {
            Ok((total, digest.finalize().into()))
        } else {
            Err(Error::Read)
        }
    }
    let first = pass(memory, governors, expected_add)?;
    if first != pass(memory, governors, expected_add)? {
        return Err(Error::Read);
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put<const N: usize>(bytes: &mut [u8], offset: usize, value: [u8; N]) {
        bytes[offset..offset + N].copy_from_slice(&value);
    }
    fn context() -> Context {
        use windows_sys::Win32::Foundation::FILETIME;
        let mut c = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut e = c;
        let mut k = c;
        let mut u = c;
        assert_ne!(
            unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) },
            0
        );
        Context {
            epoch: Epoch {
                pid: unsafe { GetCurrentProcessId() },
                created: (u64::from(c.dwHighDateTime) << 32) | u64::from(c.dwLowDateTime),
                generation: 1,
            },
            thread: unsafe { GetCurrentThreadId() },
            module_base: 0x180000000,
        }
    }
    // Uses real allocated current-process pages and ReadProcessMemory. These
    // are layout specimens, never a pretend Core function or live game result.
    #[test]
    fn actual_current_process_reader_captures_one_same_governor_receipt() {
        for fault in 0..5 {
            let context = context();
            let mut bytes = vec![0u8; 0x1000];
            let p = bytes.as_ptr() as u64;
            let ai = p;
            let player = p + 0x200;
            let governors = p + 0x400;
            let node = p + 0x600;
            let governor = p + 0x700;
            let flags = p + 0x900;
            let wrapper = p + 0xa00;
            let op = p + 0xb00;
            put(&mut bytes, 0x98, player.to_le_bytes());
            put(&mut bytes, 0x2d8, 1u32.to_le_bytes());
            put(&mut bytes, 0x470, player.to_le_bytes());
            put(&mut bytes, 0x458, node.to_le_bytes());
            put(&mut bytes, 0x600, governor.to_le_bytes());
            put(&mut bytes, 0x700, 77u32.to_le_bytes());
            put(&mut bytes, 0x704, 8u32.to_le_bytes());
            put(&mut bytes, 0x718, 1u32.to_le_bytes());
            put(&mut bytes, 0x768, flags.to_le_bytes());
            put(&mut bytes, 0x770, 16u32.to_le_bytes());
            put(&mut bytes, 0x778, 16u32.to_le_bytes());
            bytes[0x903] = 1;
            put(&mut bytes, 0xa18, 1u32.to_le_bytes());
            put(&mut bytes, 0xa20, 0x7d3da99du32.to_le_bytes());
            put(&mut bytes, 0xb00, 0x7d3da99du32.to_le_bytes());
            let row = |sequence: u32, rcx, rdx, r8, site| PromotionObservation {
                sequence: sequence * 10,
                thread_id: context.thread,
                rcx,
                rdx,
                r8,
                r9: 0,
                return_address: context.module_base + site,
            };
            let def = PaymentDefinition {
                governor_index: 8,
                cost: 2,
                base: 3,
                titles: [Some(4), None],
            };
            let m = CurrentProcessMemory;
            assert!(context.matches_current_process());
            assert_eq!(m.copy::<8>(1), None);
            let mut capture = Capture::begin(
                context,
                context.module_base + CALLBACK_ENTRY,
                row(1, ai, 2, 0, 0x401537),
                def,
                4,
                &m,
            )
            .unwrap();
            capture
                .callee(context, row(2, governors, governor, 4, AI_CHECK_RETURN), &m)
                .unwrap();
            let spent = row(3, governors, 0, 0, 0x2ddbd0);
            capture.spent_enter(context, spent).unwrap();
            let returned = ReturnObservation {
                raw: PromotionObservation {
                    sequence: spent.sequence + 1,
                    ..spent
                },
                rip: spent.return_address,
                rax: 1,
            };
            capture = match capture.spent_return(context, returned, &m).unwrap() {
                Captured::Continue(c) => c,
                Captured::Receipt(_) => panic!("premature receipt"),
            };
            capture
                .returned(
                    context,
                    row(4, 0, 0, 0, 0),
                    context.module_base + AI_CHECK_RETURN,
                    1,
                )
                .unwrap();
            capture
                .filter_entry(context, row(5, 1, op, 0x123400, 0x4a7ef5), &m)
                .unwrap();
            capture
                .returned(
                    context,
                    row(6, 0, 0, 0, 0),
                    context.module_base + 0x4a7ef5,
                    1,
                )
                .unwrap();
            capture
                .execute_prefix(
                    context,
                    row(7, wrapper, 0, 0, 0),
                    context.module_base + EXECUTE_CALL,
                    &m,
                )
                .unwrap();
            capture
                .callee(
                    context,
                    row(8, governors, governor, 4, OFFICIAL_CHECK_RETURN),
                    &m,
                )
                .unwrap();
            capture
                .returned(
                    context,
                    row(9, 0, 0, 0, 0),
                    context.module_base + OFFICIAL_CHECK_RETURN,
                    1,
                )
                .unwrap();
            capture
                .callee(context, row(10, governors, 8, 4, APPROVAL_RETURN), &m)
                .unwrap();
            bytes[0x904] = 1;
            capture
                .returned(
                    context,
                    row(11, 0, 0, 0, 0),
                    context.module_base + APPROVAL_RETURN,
                    1,
                )
                .unwrap();
            match fault {
                1 => bytes[0x904] = 0,
                2 => bytes[0x905] = 1,
                3 => put(&mut bytes, 0x718, 2u32.to_le_bytes()),
                _ => {}
            }
            let spent = row(12, governors, 0, 0, 0x713cfe);
            capture.spent_enter(context, spent).unwrap();
            let returned = ReturnObservation {
                raw: PromotionObservation {
                    sequence: spent.sequence + 1,
                    ..spent
                },
                rip: spent.return_address,
                rax: if fault == 4 { 3 } else { 2 },
            };
            let result = capture.spent_return(context, returned, &m);
            if fault != 0 {
                assert!(result.is_err(), "capture fault {fault}");
                continue;
            }
            let receipt = match result.unwrap() {
                Captured::Receipt(r) => r,
                Captured::Continue(_) => panic!("missing receipt"),
            };
            assert_eq!(receipt.spent_after, 2);
            assert_eq!(receipt.own_after.has(4), Some(true));
            assert_eq!(receipt.ai_check.rdx, governor);
            assert_eq!(receipt.approval.rdx, 8);
            assert!(!receipt.payment_authority);
            assert_eq!(receipt.spent_before_call.returned.rax, 1);
            assert_eq!(receipt.spent_after_call.returned.rax, 2);
            let artifact = receipt.artifact();
            assert_eq!(artifact["ai_check"]["rdx"].as_u64(), Some(governor));
            assert_eq!(artifact["approval"]["rdx"].as_u64(), Some(8));
            assert_eq!(artifact["spent_after_call"]["returned"]["original_rax"], 2);
            assert_eq!(artifact["full_cost_completion"], false);
        }
    }
    #[test]
    fn actual_os_context_rejects_other_pid_creation_and_thread() {
        let good = context();
        assert!(good.matches_current_process());
        for case in 0..4 {
            let mut bad = good;
            match case {
                0 => bad.epoch.pid += 1,
                1 => bad.epoch.created += 1,
                2 => bad.thread += 1,
                _ => bad.epoch.generation = 0,
            };
            assert!(!bad.matches_current_process());
        }
    }

    #[test]
    fn capture_requires_natural_caller_and_poisoned_result_cannot_be_reused() {
        let c = context();
        let mut memory = vec![0u8; 1024];
        let base = memory.as_ptr() as u64;
        put(&mut memory, 0x98, (base + 0x100).to_le_bytes());
        put(&mut memory, 0x1d8, 1u32.to_le_bytes());
        let def = PaymentDefinition {
            governor_index: 8,
            cost: 2,
            base: 3,
            titles: [Some(4), None],
        };
        let mut row = PromotionObservation {
            sequence: 1,
            thread_id: c.thread,
            rcx: base,
            rdx: 2,
            r8: 0,
            r9: 0,
            return_address: c.module_base + 0x401537,
        };
        let m = CurrentProcessMemory;
        row.return_address += 1;
        assert!(matches!(
            Capture::begin(c, c.module_base + CALLBACK_ENTRY, row, def, 4, &m),
            Err(Error::Context)
        ));
        row.return_address -= 1;
        let mut capture =
            Capture::begin(c, c.module_base + CALLBACK_ENTRY, row, def, 4, &m).unwrap();
        row.sequence = 2;
        assert_eq!(
            capture.returned(c, row, c.module_base + AI_CHECK_RETURN, 0),
            Err(Error::Filter)
        );
        row.sequence = 3;
        assert_eq!(
            capture.returned(c, row, c.module_base + AI_CHECK_RETURN, 1),
            Err(Error::Missing)
        );
    }
}
