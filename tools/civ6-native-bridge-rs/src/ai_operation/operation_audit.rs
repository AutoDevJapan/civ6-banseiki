//! Offline operation registry/serialized-queue evidence. No process access,
//! operation sender, runtime hook, or native shutdown authorization.
#[path = "operation_audit/fixed.rs"]
mod fixed;
#[path = "operation_audit/pe.rs"]
mod pe;
#[path = "operation_audit/secondary.rs"]
mod secondary;
#[path = "operation_audit/secondary_fixed.rs"]
mod secondary_fixed;
use pe::{Function, Pe};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const CORE_SHA: &str = "092abfbb2176f8df04b5212f819dc269dd83aea866ff05aaf489904a1ff7c41f";
struct Fragment {
    begin: u32,
    end: u32,
    unwind: u32,
    hex: &'static str,
}
struct Frame {
    begin: u32,
    end: u32,
    sha: &'static str,
    // Original bytes allow a portable synthetic-PE positive control. Production
    // compares the independent fixed SHA, not bytes generated from its input.
    #[cfg_attr(not(test), allow(dead_code))]
    specimen: &'static str,
    fragments: &'static [Fragment],
}
struct Leaf {
    rva: u32,
    hex: &'static str,
}
fn all_frames() -> impl Iterator<Item = &'static Frame> {
    fixed::FRAMES.iter().chain(secondary_fixed::FRAMES)
}
fn all_leaves() -> impl Iterator<Item = &'static Leaf> {
    fixed::LEAVES.iter().chain(secondary_fixed::LEAVES)
}
fn unhex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return Err("invalid fixed hex length/encoding".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|p| u8::from_str_radix(&s[p..p + 2], 16).map_err(|_| "invalid fixed hex".into()))
        .collect()
}
fn exact(pe: &Pe<'_>, rva: u32, hex: &str) -> Result<(), String> {
    let expected = unhex(hex)?;
    if pe.span(rva, expected.len(), true)? != expected {
        return Err(format!("operation instruction bytes differ at {rva:#x}"));
    }
    Ok(())
}
fn complete_frame(pe: &Pe<'_>, spec: &Frame) -> Result<Value, String> {
    let parts: Vec<_> = pe
        .functions
        .iter()
        .copied()
        .filter(|f| f.begin >= spec.begin && f.begin < spec.end)
        .collect();
    if parts.len() != spec.fragments.len() || parts.is_empty() {
        return Err("operation complete frame partition differs".into());
    }
    let mut next = spec.begin;
    for (f, expected) in parts.iter().zip(spec.fragments) {
        if *f
            != (Function {
                begin: expected.begin,
                end: expected.end,
                unwind: expected.unwind,
            })
            || f.begin != next
            || f.end > spec.end
        {
            return Err("operation pdata fragment/gap differs".into());
        }
        let unwind = unhex(expected.hex)?;
        if pe.span(f.unwind, unwind.len(), false)? != unwind || pe.chain_root(*f)? != parts[0] {
            return Err("operation complete unwind/CHAININFO differs".into());
        }
        next = f.end;
    }
    if next != spec.end
        || format!(
            "{:x}",
            Sha256::digest(pe.span(spec.begin, (spec.end - spec.begin) as usize, true)?)
        ) != spec.sha
    {
        return Err(format!(
            "operation complete code SHA/extent differs at {:#x}",
            spec.begin
        ));
    }
    Ok(json!({"begin":spec.begin,"end":spec.end,"sha256":spec.sha,
        "fragments":spec.fragments.iter().map(|f|json!({"begin":f.begin,
            "end":f.end,"unwind_rva":f.unwind,"unwind_hex":f.hex})).collect::<Vec<_>>()}))
}
// The operand is independently decoded after verifying the instruction bytes.
fn relative(
    pe: &Pe<'_>,
    rva: u32,
    len: usize,
    displacement: usize,
    target: u32,
) -> Result<Value, String> {
    let b = pe.span(rva, len, true)?;
    let d = if len == 2 && displacement == 1 {
        i32::from(b[1] as i8)
    } else {
        i32::from_le_bytes(pe::at(b, displacement, 4)?.try_into().unwrap())
    };
    let computed = i64::from(rva) + len as i64 + i64::from(d);
    if computed != i64::from(target) {
        return Err(format!("operation relative target differs at {rva:#x}"));
    }
    Ok(json!({"rva":rva,"instruction_bytes":len,"computed_target_rva":target}))
}
const TABLES: &[(u32, [u32; 7])] = &[
    (
        0x9be250,
        [
            0x39f10, 0x35f00, 0x3fa120, 0x35f70, 0x38eb0, 0x35fe0, 0x38e70,
        ],
    ),
    (
        0x9be370,
        [
            0x360b0, 0x360e0, 0x3fa120, 0x36180, 0x363d0, 0x36420, 0x36140,
        ],
    ),
    (
        0x9bec58,
        [
            0x39f10, 0x39880, 0x3fa120, 0x39920, 0x397c0, 0x39df0, 0x398f0,
        ],
    ),
];

/// A strict complete-file SHA gate precedes all PE and ABI interpretation.
pub fn audit(core: &[u8]) -> Result<Value, String> {
    if format!("{:x}", Sha256::digest(core)) != CORE_SHA {
        return Err("not the steam-26407122 Core SHA256; no operation ABI interpretation".into());
    }
    inspect(&Pe::parse(core)?)
}

fn inspect(pe: &Pe<'_>) -> Result<Value, String> {
    let frames = all_frames()
        .map(|f| complete_frame(pe, f))
        .collect::<Result<Vec<_>, _>>()?;
    for leaf in all_leaves() {
        exact(pe, leaf.rva, leaf.hex)?;
        let end = leaf.rva + (leaf.hex.len() / 2) as u32;
        if pe
            .functions
            .iter()
            .any(|f| f.begin < end && f.end > leaf.rva)
        {
            return Err("bounded leaf unexpectedly covered by pdata".into());
        }
    }
    let mut slots = Vec::new();
    for (base, targets) in TABLES {
        for (i, target) in targets.iter().enumerate() {
            let slot = base + i as u32 * 8;
            pe.pointer(slot, *target)?;
            slots.push(json!({"table_rva":base,"slot_offset":i*8,"slot_rva":slot,
                "target_rva":target,"relocation":"DIR64"}));
        }
    }
    for (rva, hex) in BLOCKS {
        exact(pe, *rva, hex)?;
    }
    let references = REFERENCES
        .iter()
        .map(|&(rva, len, offset, target)| relative(pe, rva, len, offset, target))
        .collect::<Result<Vec<_>, _>>()?;
    let secondary_queue = secondary::inspect(pe)?;
    let registry = json!({
        "register_entry_rva":0x37e90,"lookup_root_global_rva":0xb8a9d0,
        "sentinel_rva":0xb8a9c0,"node_type_id_offset":0x20,"node_instance_offset":0x28,
        "factory_call_rva":0x37ede,"factory_slot":8,"factory_store_rva":0x37f55,
        "registration_examples":[{"default_constructor":0x35ed0,"call":0x37016,"register_call":0x3701e},
            {"default_constructor":0x39850,"call":0x37005,"register_call":0x3700d}],
        "factory_semantics":"checked +8 factories allocate a fresh default instance and initialize its table/type fields; they do not copy the input prototype payload in these complete bodies",
            "lifetime_limit":"registry stores an instance across calls; ownership, an actual executed vtable, absence of replacement/aliasing/reentry, World birth and registry destruction require runtime/lifetime evidence"
    });
    let execute_consumer = json!({
        "whole_entry_rva":0x37b50,"input_player":"ECX saved in R12d",
        "input_stream":"entry RDX saved in RBX; first four bytes decoded as registry type ID",
        "lookup":"37BC3..37C06 searches B8A9D0 by node+20 with sentinel B8A9C0; R14 is the selected node",
        "deserialize_receiver_load_rva":0x37c0c,"deserialize_call_rva":0x37c23,
        "deserialize_target_rva":0x33bd0,"payload_reader_call_rva":0x33c2d,"payload_reader_slot":0x28,
        "execute_receiver_reload_rva":0x37e56,"execute_call_rva":0x37e64,"execute_return_rva":0x37e67,
        "execute_slot":0x18,"flag_global_rva":0xb8aa18,"flag_set_rva":0x37e5a,"flag_clear_rva":0x37e77,
        "scope":"both loads use the same selected node+28 after normal callees preserve R14; pointer and vtable equality across calls is not certified",
        "not_a_packet_clone":"consumer overwrites the registered instance using its +28 reader then reloads node+28 for +18; no per-packet +8 factory call in the complete consumer",
        "logging_entry_excluded_from_execution_witness":0x366b0
    });
    let queued_path = json!({
        "enqueue_adapter_rva":0x590810,"enqueue_entry_rva":0x373e0,
        "enqueue_input":"ECX saved in R14d is a player/source scalar; RDX saved in RSI is a serialized stream, not an operation pointer",
        "size_guard_call_rva":0x373fe,"size_guard_slot":0x10,"zero_size_branch_rva":0x37407,
        "zero_size_target_rva":0x3767f,
        "allocator_entry_rva":0x994970,"allocator_call_rva":0x37435,"allocator_return_rva":0x3743a,
        "allocation_size":0x38,"allocation_result":"RAX=node; RDX receives it at3743A",
        "link_preparation_rva":0x37459,"first_shared_link_store_rva":0x37472,"tail_publication_rva":0x37475,
        "list_head_rva":0xb8a9e8,"list_tail_rva":0xb8a9f0,"node_player_offset":0x10,
        "stream_resize_call_rva":0x37481,"stream_source_call_rva":0x3748d,"stream_copy_call_rva":0x3749b,
        "copy_target_rva":0x990d00,"copy_source_target_rva":0x956030,
        "second_list":"when B8AA19 permits, a separate node is allocated at374C9 and published through B8AA00/B8AA08 at37506/37509 before its second stream copy3752F",
        "drain_adapter_rva":0x590790,"drain_entry_rva":0x372f0,"consumer_call_rva":0x37353,
        "consumer_return_rva":0x37358,
        "dequeue_binding":"372F0 retains queue node in RBX; [node+10] becomes ECX and [node+18..+20] is adapted to entry RDX before37353; source RBX remains nonvolatile through37B50",
        "normal_cleanup":"after consumer normal return, head is reloaded at37358, unlink stores37366/37370 then stream/node storage release37388/3739D; head identity stability is not proved by a reload",
        "publication_precedes_payload_copy":true,
        "reservation_frontier":"reserve at enqueue entry before native calls; bind allocation RAX at caller3743A before any shared link; do not wait for enqueue return",
        "source_epoch":"carry actual issuing epoch with the queue-node reservation into dequeue and full consumer/SEH; no current-generation snapshot at prototype factory or execute entry",
            "second_list_limit":"second allocation is a distinct record; see secondary_queue for checked copy/serialized-ingestion/free paths; runtime World epoch and completeness remain unestablished"
    });
    let immediate_path = json!({
        "adapter_rva":0x590820,"target_rva":0x37b50,
        "arguments":"EDX becomes ECX, R8 becomes RDX, then tailcall; admission at this adapter is separate from queued-node provenance",
        "queue_node_not_implied":"RBX is meaningful as queue identity only for the verified drain caller37358, not for all37B50 invocations"
    });
    let allocator_adapter_contract = json!({
        "entry_rva":0x994970,"incoming_stack_argument_count":2,"incoming_stack_offsets":[0x28,0x30],
        "displaced_instruction":"44 8B 44 24 30: first instruction reads sixth incoming argument",
        "tail_rva":0x994988,"tail_target_rva":0x990900,
        "requirement":"copy original-entry stack arguments5/6 to the forwarded original-call stack before executing displaced code; preserve all registers/flags/LastError and normal return/real SEH; bind RAX only on normal return",
        "implemented_by_this_audit":false
    });
    Ok(json!({
        "schema":"native-operation-domain-static-v2","image_revision":"steam-26407122",
        "core_sha256":CORE_SHA,"whole_frames":frames,"relocated_vtable_slots":slots,
        "computed_references":references,"registry":registry,"execute_consumer":execute_consumer,
        "queued_path":queued_path,"immediate_path":immediate_path,
        "allocator_adapter_contract":allocator_adapter_contract,
        "secondary_queue":secondary_queue,
        "not_established_by_this_audit":[
            "executed operation table/registry identity and equality between deserialize and execute reloads",
            "World epoch for any constructor, prototype registration, player scalar or stream payload",
            "all enqueue/immediate/copy/replay paths or all indirect consumers; checked secondary paths are a partial static slice",
            "producer seal, native pending count, cancel/reissue/ABA settlement or all-domain join",
            "safety of queue publication before payload copy under concurrency",
            "6-argument allocator gateway ABI/SEH and runtime reservation installation",
            "native shutdown/Close/Rebuild authority or original seven product acceptance conditions"
        ],
        "certificate_issued":false,"product_acceptance":false,
        "structural_declarations":{"runtime_calls":0,"game_runs":0,
            "basis":"this binary borrows file buffers only; these are not measured runtime counters"}
    }))
}

// Contiguous instruction intervals support field-flow and conditional-branch
// statements; complete frame SHA/pdata above additionally cover intervening code.
#[path = "operation_audit/flow.rs"]
mod flow;
use flow::{BLOCKS, REFERENCES};

#[cfg(test)]
#[path = "operation_audit/tests.rs"]
mod tests;
