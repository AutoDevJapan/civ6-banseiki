//! Typed World record registration and lifecycle callback evidence.
#[path = "world_fixed.rs"]
pub(super) mod fixed;
#[path = "world_flow.rs"]
pub(super) mod flow;

use super::{inspect_image, pe::Pe, Value};
use serde_json::json;

pub(super) fn inspect(core: &Pe<'_>) -> Result<Value, String> {
    let evidence = inspect_image(
        core,
        fixed::FRAMES,
        flow::BLOCKS,
        flow::REFERENCES,
        flow::SLOTS,
        true,
    )?;
    for (rva, expected) in [
        (0xa18d90, &b"UnitManager\0"[..]),
        (0xa45238, &b"Notifications\0"[..]),
        (0x9c4860, &b"ObjectPairing\0"[..]),
    ] {
        if core.span(rva, expected.len(), false)? != expected {
            return Err(format!("World record name differs at {rva:#x}"));
        }
    }
    Ok(json!({
        "evidence":evidence,
        "world_registration":{
            "world_table":0xa45160,"registration_slot":0x18,"registration_entry":0x750a70,
            "world_global_getter":0x951550,"world_global":0xb8aa68,
            "registration_entry_shared":0x63da80,"arguments":"RCX=World, RDX=name, R8=factory functor",
            "descriptor_fields":{"key_hash":0,"name":8,"functor_storage":0x10,"active_functor":0x48,"stride":0x50},
            "factory_vector":{"begin":0x228,"end":0x230,"capacity":0x238},
            "existing_key_flow":"63DAE3 compares the name hash; existing record updates its name and replaces the copied functor; new record is published by adding50 to vector end at63DDD6",
            "return_value":"EAX=record index, or FFFFFFFF for null name; not a Host native session ID",
            "provenance_limit":"registration wrappers obtain World from Core globalB8AA68; native observation must match that returned pointer to the already bound World incarnation"},
        "materialization":{
            "entry":0x63cc10,"arguments":"RCX=World",
            "first_8_hex":"48895c241048896c","stolen_lengths":[5],"continuation":0x63cc15,
            "source_load":0x63cc6f,"factory_call":0x63cc9f,"factory_return":0x63cca2,
            "factory_receiver":"RCX=descriptor+48 active functor; fixed functor virtual+10=751680 tail-jumps to functor+8 saved factory pointer",
            "bind_entry":0x987b0,"bind_caller_return":0x63ccb2,
            "bind_arguments":"RCX=World+8 vector, RDX=new object from factory RAX, R8=descriptor name",
            "bind_first_8_hex":"48895c2408488974","bind_stolen_lengths":[5],"bind_continuation":0x987b5,
            "publication_helper":0x507c0,"publication_call":0x9888b,
            "object_publish":0x50877,"published_vector_end":0x50898,"record_stride":0x120,
            "ownership":"507C0 moves the temporary first qword into the new record and zeros the temporary;987B0 destroys the temporary only if still nonzero",
            "runtime_contract":"under an active63CC10 full frame, bind the returned factory object to inherited typed World before original987B0 publishes it; keep object identity through record moves and callbacks; null factory results cannot create coverage"},
        "object_pairing_chain":{
            "name":"ObjectPairing","name_rva":0x9c4860,
            "functor_table":0xa455a8,"factory_pointer_store":0x750af9,"factory":0x640d10,
            "registration_wrapper":0x641020,"registration_call":0x750b0c,
            "record_index_global":0xb69400,"name_hash_global":0xb8fc30,
            "constructor":0x640140,"object_table":0xa22c98,
            "attach_slot":8,"attach_entry":0x640f80,"attach_effect":"mov [object+8],RDX; ret",
            "shutdown_slot":0x10,"shutdown_entry":0x6412a0,"shutdown_leaf":"mov RAX,[RCX]; jmp[RAX+18]",
            "typed_shutdown_target":0x641240,
            "shutdown_flow":"obtain object+A8 state via72A920, recursively free its tree viaB0420, reset tree fields; obtain object+E8 state, tail5167A0 to recursively free its tree via7BB80 and reset fields",
            "terminal_free":"36E20 transfers RDX to RCX and tails9908F0; no Host session identity or join argument is established by these memory-release calls",
            "limit":"a complete concrete World record chain has been exhausted to native storage release; this callback does not furnish a same-session Host845D30/98C650 receipt"},
        "other_fixed_examples":[
            {"name":"UnitManager","factory":0x64f9b0,"table":0xa23c60,"shutdown_slot":0x10,"shutdown":0x8972d0,"body_hex":"c20000"},
            {"name":"Notifications","factory":0x7529d0,"table":0xa45e48,"shutdown_slot":0x10,"shutdown":0x3d6e0,
             "flow":"clear per-player notification lists; tail object virtual+68=752B00 for notification reset functions; not assigned Host session join semantics"}],
        "shutdown":{
            "entry":0x502e0,"original_caller_return":0x63e1b8,
            "record_receiver":"RCX=record first qword from World+8..+10, reverse order, stride120",
            "vcall":0x50358,"slot":0x10,
            "adapter_contract":"under typed World shutdown, retain explicit World-record-object-table identity at callback entry and through normal/SEH full return; any native stop/join receipt must identify the same bound session/transport independently",
            "game_callback_query":0x63de80,"game_field":0x270,"game_slot":0x68,
            "fixed_game_table":0xa44f70,"fixed_game_target":0x8972d0,
            "query_limit":"63DE80 passes constantBD0269DD to CGame virtual+68; pinned A44F70 target contains only RET imm16(0), with no native ID lookup or assignment; this is not a scalar-zero return claim"},
        "remaining_exact_edge":"a retained World incarnation to Host registry lifetime/nativeID/session/transport incarnation, plus the actual caller that closes and joins that same transport before Core380B0; descriptor index/hash and502E0 callback return cannot substitute",
        "adapter_runtime_installed":false,"certificate_issued":false
    }))
}
