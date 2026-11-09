//! Offline evidence for serialized-message admission and teardown ordering.
//! No process access, sender, hook installation or lifecycle authorization.
#[path = "session_audit/fixed.rs"]
mod fixed;
#[path = "session_audit/flow.rs"]
mod flow;
#[path = "session_audit/lease.rs"]
mod lease;
#[path = "session_audit/pe.rs"]
mod pe;
#[path = "session_audit/snapshot.rs"]
mod snapshot;
#[path = "session_audit/view_worker.rs"]
mod view_worker;
#[path = "session_audit/world.rs"]
mod world;

use pe::{Function, Pe};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const CORE_SHA: &str = "092abfbb2176f8df04b5212f819dc269dd83aea866ff05aaf489904a1ff7c41f";
const HOST_SHA: &str = "d1a0cfd439e7451be515f47595cfa0b6b983f56d1faf4ea434a4ce3b21491ebe";
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
    #[cfg(test)]
    specimen: &'static str,
    fragments: &'static [Fragment],
}
struct Block {
    rva: u32,
    hex: &'static str,
}
struct Reference {
    rva: u32,
    prefix: &'static str,
    length: usize,
    displacement: usize,
    target: u32,
    kind: &'static str,
}
fn unhex(s: &str) -> Result<Vec<u8>, String> {
    if !s.is_ascii() || !s.len().is_multiple_of(2) {
        return Err("session fixed hex encoding/length".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|p| u8::from_str_radix(&s[p..p + 2], 16).map_err(|_| "session fixed hex".into()))
        .collect()
}
fn exact(pe: &Pe<'_>, rva: u32, hex: &str) -> Result<(), String> {
    let expected = unhex(hex)?;
    if pe.span(rva, expected.len(), true)? != expected {
        return Err(format!("session bytes differ at {rva:#x}"));
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
    if parts.is_empty() || parts.len() != spec.fragments.len() {
        return Err("session complete pdata partition count".into());
    }
    let mut next = spec.begin;
    for (actual, wanted) in parts.iter().zip(spec.fragments) {
        if *actual
            != (Function {
                begin: wanted.begin,
                end: wanted.end,
                unwind: wanted.unwind,
            })
            || actual.begin != next
            || actual.end > spec.end
        {
            return Err("session complete pdata partition/extent".into());
        }
        let unwind = unhex(wanted.hex)?;
        if pe.span(actual.unwind, unwind.len(), false)? != unwind
            || pe.chain_root(*actual)? != parts[0]
        {
            return Err("session complete unwind/CHAININFO".into());
        }
        next = actual.end;
    }
    if next != spec.end
        || format!(
            "{:x}",
            Sha256::digest(pe.span(spec.begin, (spec.end - spec.begin) as usize, true)?)
        ) != spec.sha
    {
        return Err(format!("session complete frame SHA at {:#x}", spec.begin));
    }
    Ok(json!({"begin":spec.begin,"end":spec.end,"sha256":spec.sha,
        "fragments":spec.fragments.iter().map(|f|json!({"begin":f.begin,"end":f.end,
            "unwind_rva":f.unwind,"unwind_hex":f.hex})).collect::<Vec<_>>()}))
}
fn target(rva: u32, code: &[u8], length: usize, offset: usize) -> Result<u32, String> {
    if code.len() != length {
        return Err("session relative length".into());
    }
    let delta = if length == 2 && offset == 1 {
        i64::from(code[1] as i8)
    } else {
        i64::from(i32::from_le_bytes(
            pe::at(code, offset, 4)?.try_into().unwrap(),
        ))
    };
    u32::try_from(i64::from(rva) + length as i64 + delta)
        .map_err(|_| "session relative target bounds".into())
}
fn references(pe: &Pe<'_>, refs: &[Reference]) -> Result<Vec<Value>, String> {
    refs.iter()
        .map(|r| {
            let b = pe.span(r.rva, r.length, true)?;
            if !b.starts_with(&unhex(r.prefix)?)
                || target(r.rva, b, r.length, r.displacement)? != r.target
            {
                return Err(format!(
                    "session {} target/operation at {:#x}",
                    r.kind, r.rva
                ));
            }
            if r.kind == "rip_global" {
                pe.virtual_data_address(r.target, 8)?;
            } else if r.kind == "rip_data" {
                pe.offset(r.target, 1)?;
            } else {
                pe.span(r.target, 1, true)?;
            }
            Ok(json!({"rva":r.rva,"kind":r.kind,"computed_target_rva":r.target}))
        })
        .collect()
}
fn inspect_image(
    pe: &Pe<'_>,
    frames: &[Frame],
    blocks: &[Block],
    refs: &[Reference],
    slots: &[(u32, u32)],
    dll: bool,
) -> Result<Value, String> {
    let frames = frames
        .iter()
        .map(|f| complete_frame(pe, f))
        .collect::<Result<Vec<_>, _>>()?;
    for b in blocks {
        exact(pe, b.rva, b.hex)?;
    }
    let refs = references(pe, refs)?;
    let slots=slots.iter().map(|&(s,t)|{
        pe.pointer(s,t,dll)?;
        Ok(json!({"slot":s,"target":t,"pointer_basis":if dll {"DIR64"}else{"preferred_image_disk_pointer"}}))
    }).collect::<Result<Vec<_>,String>>()?;
    Ok(
        json!({"whole_frames":frames,"computed_references":refs,"slots":slots,
        "instruction_blocks":blocks.iter().map(|b|json!({"rva":b.rva,"hex":b.hex})).collect::<Vec<_>>()}),
    )
}
fn pair(core: &[u8], host: &[u8]) -> Result<(), String> {
    if format!("{:x}", Sha256::digest(core)) != CORE_SHA
        || format!("{:x}", Sha256::digest(host)) != HOST_SHA
    {
        return Err("not both pinned steam-26407122 files; no session ABI interpretation".into());
    }
    Ok(())
}

/// Both borrowed complete-file buffers are pinned before either ABI is read.
pub fn audit(core: &[u8], host: &[u8]) -> Result<Value, String> {
    pair(core, host)?;
    inspect(&Pe::parse(core)?, &Pe::parse_host(host)?)
}
fn inspect(core: &Pe<'_>, host: &Pe<'_>) -> Result<Value, String> {
    let c = inspect_image(
        core,
        fixed::CORE_FRAMES,
        flow::CORE_BLOCKS,
        flow::CORE_REFERENCES,
        flow::CORE_SLOTS,
        true,
    )?;
    let h = inspect_image(
        host,
        fixed::HOST_FRAMES,
        flow::HOST_BLOCKS,
        flow::HOST_REFERENCES,
        flow::HOST_SLOTS,
        false,
    )?;
    let native_lease = lease::inspect(host)?;
    let world_records = world::inspect(core)?;
    let snapshot_stream = snapshot::inspect(host)?;
    let view_worker = view_worker::inspect(host)?;
    let admission = json!({
        "context_table":0xa101e8,"context_constructor":0x58fb40,
        "enqueue":{"slot":0x230,"adapter":0x590810,"target":0x373e0},
        "immediate":{"slot":0x228,"adapter":0x590820,"target":0x37b50},
        "drain":{"slot":0x238,"adapter":0x590790,"target":0x372f0},
        "replay":{"slot":0x270,"adapter":0x590730,"target":0x37690},
        "adapter_receiver_limit":"590810/590820 overwrite entry RCX with EDX and RDX with R8; the Context receiver is lost before the target; 590730 similarly replaces RCX by RDX",
        "runtime_type_limit":"fixed table/constructor relation only; actual vtable and Context/World continuity need native provenance"});
    let message = json!({
        "host_table":0xfd0d38,"wire_type":3,"registration_entry":0x96b0,
        "registration_factory":0x265090,"reader_slot":0x18,"reader":0x265f50,
        "dispatch_slot":0x20,"dispatch":0x265660,"destructor_slot":0,"destructor":0x264ef0,
        "factory_flow":"96CC sets R8d=3,96D2 loads factory265090,96DD tails7FB560 which stores callback at its matching key node+8; factory allocates50 bytes and installs FD0D38",
        "reader_flow":"265F50 retains message RCX in RBX and incoming stream RDX in RDI;7FB310 reads base header; tail264790 fills message+38 byte vector",
        "base_header":"7FB310 reads stream fields into message+30,+20,+1C,+28,+2C; no world-generation interpretation assigned to these fields",
        "payload_fields":{"source_player":0x24,"byte_buffer":0x38,"byte_count":0x48},
        "dispatch_flow":"265680 loads [message+38],265689 loads count+48, constructs stream;2656AE callsF7BF0;2656C5 loads player+24;2656C8 invokes Context virtual+230",
        "current_context_lookup":{"getter":0xf7bf0,"global":0x22d70c0,
            "cold_factory_slot":0x22d74e8,"flow":"cached current pointer, otherwise call current factory and store result; no original packet-session generation argument"},
        "admission_guard":{"call":0x265670,"target":0x27dee0,"leaf":0xf4c60,
            "flow":"27DEE4 callsF4150 then tailF4C60 reads byte[current-state+B]; this is not proof of old packet rejection or network stop/join"},
        "exact_lifetime_fact":"the full dispatch body obtains current Context after message bytes already exist; it does not derive Context from a retained message field",
        "limits":"message+24 player and wire header values are not World/session epochs; current Context pointer or successful dispatch is not a birth receipt"});
    let transport = json!({
        "decoder_entry":0x7f9c30,"raw_wrapper":0x7fabf0,
        "factory_lookup_call":0x7fa08c,"factory_lookup":0x7fb600,
        "reader_call":0x7fa0a6,"reader_return":0x7fa0aa,
        "reader_receiver":"R15 holds returned message from factory; RCX=message,RDX=RBX local stream; call[[message]+18]",
        "lookup_limit":"7FB600 is a bounded leaf verified separately; its returned callback creates a default message, with no session-generation argument",
        "queued_pump_entry":0x83d570,"queued_decode":0x83d819,
        "queued_storage":"entry RCX saved as RBX; native queue fields E0/E8/F0/F8; record RDI contains bytes+0/+8 and scalars+20/+24",
        "queued_dispatch":0x83d882,
        "queued_dispatch_receiver":"RCX=entry pump RBX; RDX=decoder RAX retained in RSI; call[[RBX]+238]; this is a transport object's slot, not evidence of Core Context type",
        "incoming_entry":0x860000,"incoming_table":0x10494a8,"incoming_slot":0x28,"incoming_table_slot":0x10494d0,
        "incoming_receiver":"entry RCX is connection-facing interface; [RCX+8] indexes global235B878 via7F9960; R14 retains the returned session-facing object",
        "incoming_decode_calls":[0x860073,0x8600dd],"incoming_dispatch_calls":[0x8600f7,0x860136],
        "incoming_post_dispatch_destroy":[0x860109,0x860147],
        "session_dispatch":0x842030,
        "limits":"native_lease fixes the registration, callback and session-dispatch types; no World epoch is inferred from registry ID or current Context"});
    let shutdown = json!({
        "entry":0x63e160,"world_table":0xa45160,"world_constructor":0x750600,
        "early_flag_stores":[0x63e176,0x63e182],
        "state_helper_call":0x63e1a3,"state_helper":0x64a6d0,"state_receiver":"entry World+B0",
        "state_helper_limit":"complete body updates a keyed state node/value; no native wait or packet generation is certified",
        "world_virtual_call":0x63e1b5,"slot":0x48,"fixed_target":0x502e0,
        "target_body":"reverse traversal of receiver+8..+10 in120h-sized records; each selected record's first pointer receives virtual+10 at50358",
        "virtual_type_limit":"conditional on the actual World having A45160; callback target/type and stop/join semantics of each record are not established",
        "manager_call":0x63e1d0,"manager":0x4a1150,"wait_return":0x4a1196,
        "cleanup_call":0x63e234,"cleanup_entry":0x380b0,"cleanup_return":0x63e239,
        "first_destructive_use":0x380e3,"receiver_load":0x380d2,
        "destructive_flow":"380D2 loads registry-node+28;380DB loads its table,380DE sets EDX1,380E3 calls slot0 before queued storage release",
        "cleanup_queue_storage_frees":[0x3818a,0x3819f,0x381ea,0x381ff],
        "required_cut":"session/message/producer inflight and every active prototype reader/execute must be resolved BEFORE original380B0; cleanup-return storage drain alone is too late to protect380E3",
        "limits":"flags2A4/2A6, normal manager Wait and storage-free normal returns are distinct facts; none independently seals every producer"});
    Ok(
        json!({"schema":"native-session-boundary-static-v5","image_revision":"steam-26407122",
        "core_sha256":CORE_SHA,"host_sha256":HOST_SHA,"core_evidence":c,"host_evidence":h,
        "context_admission":admission,"message_lifetime":message,"transport":transport,
        "shutdown_order":shutdown,"adapter_contract":adapter_contract(),"native_lease":native_lease,"world_records":world_records,
        "snapshot_stream":snapshot_stream,
        "view_worker":view_worker,
        "not_established_by_this_audit":[
            "old packet/session identity across lookup, session replacement, reconnect and address reuse",
            "upstream network admission stop and same-session inflight join with native guarantees",
            "complete alternate incoming/replay paths and joins for every transport kind",
            "World generation for all operation/message constructors, deserializers and queues",
            "runtime gateway installation, exception propagation correctness or lifecycle product acceptance",
            "all-domain producer seal/copy/cancel/join and normal-runtime Rebuild authority"],
        "certificate_issued":false,"product_acceptance":false,
        "structural_declarations":{"runtime_calls":0,"game_runs":0,
            "basis":"file-buffer audit structure, not native execution measurements"}}),
    )
}
fn adapter_contract() -> Value {
    json!({"message_reader_candidate":{"entry":0x265f50,"verify_first_8_hex":"48895c2408574883",
            "stolen_lengths":[5],"continuation":0x265f55,
            "entry_arguments":"RCX=message;RDX=incoming serialized stream"},
        "message_dispatch_candidate":{"entry":0x265660,"verify_first_8_hex":"40534883ec50488b",
            "stolen_lengths":[2,4],"continuation":0x265666,"entry_argument":"RCX=message before any receiver dereference"},
        "cleanup_candidate":{"entry":0x380b0,"verify_first_8_hex":"48895c2408488974",
            "stolen_lengths":[5],"continuation":0x380b5,"required_original_caller":0x63e239},
        "requirements":[
            "reserve from an already bound upstream session/connection generation before first reader/message admission; never assign the current World generation at delayed dispatch",
            "carry opaque message identity and inherited generation through reader, queued transport, dispatch, Core enqueue/copy/replay and final message destructor",
            "a reader-entry reservation is insufficient until its upstream stream/session origin has been bound; default factory265090 has no source-session argument",
            "a sticky closed session rejects stale admission; absence of active bridge frames is not proof that the native receiver cannot publish again",
            "before original cleanup380B0 allow native execution only after producer/message and prototype-use inflight drainage; inert archives can retain separate storage reservations until original cleanup returns",
            "whole-function gateways must execute the original once and preserve input/output GPR/XMM, flags, LastError, incoming stack arguments, alignment and full-frame unwind ownership",
            "save-RBX displaced instructions need matching SAVE_NONVOL; SEH must propagate original unwind, retain unresolved publication state and never create a normal completion or seal"],
        "implemented_by_this_audit":false})
}

#[cfg(test)]
#[path = "session_audit/tests.rs"]
mod tests;
