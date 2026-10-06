//! Bounded raw rows from the existing original-call observer, never a permit.
use super::*;
use crate::ai_operation::gateway;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AiCaptureRow {
    pub sequence: u32,
    pub thread: u32,
    pub flags: u32,
    pub owner: u32,
    pub generation: u32,
    pub index: u32,
    pub promotion_count: u32,
    pub own_owner: u32,
    pub raw: gateway::Registers,
    pub owned: [u64; 64],
}
#[repr(C)]
pub struct AiCapture {
    pub state: AiRuntimeState,
    pub count: u32,
    pub reserved: u32,
    pub module_base: u64,
    pub rows: [AiCaptureRow; 128],
}
#[unsafe(no_mangle)]
pub extern "system" fn BridgeRuntimeAiCapture(input: *mut c_void) -> u32 {
    if !committed_range(
        input as usize,
        std::mem::size_of::<AiCapture>(),
        std::mem::align_of::<AiCapture>(),
        true,
    ) {
        return BridgeStatus::InvalidInput as u32;
    }
    let status = ai_runtime::BridgeRuntimeAiQuery(input);
    if status != BridgeStatus::Ok as u32 {
        return status;
    }
    let packet = unsafe { &mut *input.cast::<AiCapture>() };
    let Some(a) = ai_operation::ATTEMPT.get() else {
        return BridgeStatus::InvalidInput as u32;
    };
    if (packet.state.owner, packet.state.governor_index) != (a.subject.owner, a.subject.index) {
        return BridgeStatus::ProcessIdentity as u32;
    }
    let (rows, dropped) = gateway::observations();
    packet.count = rows.len() as u32;
    packet.module_base = gateway::observation_module_base().unwrap_or(0);
    packet.state.observations = packet.count;
    packet.state.dropped = dropped;
    for (out, row) in packet.rows.iter_mut().zip(rows) {
        *out = AiCaptureRow {
            sequence: row.sequence,
            thread: row.thread,
            flags: u32::from(row.after)
                | (u32::from(row.accepted) << 1)
                | (u32::from(row.capture_entry) << 2)
                | (u32::from(row.own.is_some()) << 3),
            owner: row.owner.unwrap_or(u32::MAX),
            generation: row.generation,
            index: row.own.map_or(u32::MAX, |p| p.governor_index),
            promotion_count: row.own.map_or(0, |p| p.count),
            own_owner: row.own.map_or(u32::MAX, |p| p.owner),
            raw: row.raw,
            owned: row.own.map_or([0; 64], |p| p.owned),
        };
    }
    status
}
