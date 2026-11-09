//! Second queue, copy publication, serialized ingestion and cleanup contracts.
use super::{exact, pe::Pe, relative};
use serde_json::{json, Value};
#[path = "secondary_flow.rs"]
mod flow;
use flow::{BLOCKS, REFERENCES};

#[cfg(test)]
pub(super) fn blocks() -> &'static [(u32, &'static str)] {
    BLOCKS
}

pub(super) fn inspect(pe: &Pe<'_>) -> Result<Value, String> {
    for (rva, hex) in BLOCKS {
        exact(pe, *rva, hex)?;
    }
    let refs = REFERENCES
        .iter()
        .map(|&(rva, len, disp, target)| relative(pe, rva, len, disp, target))
        .collect::<Result<Vec<_>, _>>()?;
    let entries = super::secondary_fixed::FRAMES.iter().map(|f| {
        let fragment = &f.fragments[0];
        let prolog_len = usize::from(pe.span(fragment.unwind, 4, false)?[1]);
        let bytes = pe.span(f.begin, prolog_len, true)?;
        Ok(json!({"entry_rva":f.begin,"whole_end_rva":f.end,
            "unwind_declared_prolog_length":prolog_len,
            "unwind_declared_prolog_hex":bytes.iter().map(|b|format!("{b:02x}")).collect::<String>(),
            "scope":"exact unwind-declared interval; split saves, branches and calls may occur inside it; not a minimal hook displacement"}))
    }).collect::<Result<Vec<_>, String>>()?;
    let enqueue_second = json!({
        "ancestor_entry_rva":0x373e0,"flag_test_rva":0x374a0,"flag_rva":0xb8aa19,
        "skip_target_rva":0x3766f,
        "allocator_call_rva":0x374c9,"allocator_return_rva":0x374ce,"allocator_rva":0x994970,
        "normal_return_node":"RAX allocation result moved to RDX at374CE; RBX becomes node+10 at374D1",
        "first_shared_link_store_rva":0x37506,"tail_store_rva":0x37509,
        "head_rva":0xb8aa00,"tail_rva":0xb8aa08,
        "record_player_store_rva":0x37512,"payload_resize_call_rva":0x37515,
        "payload_copy_call_rva":0x3752f,
        "identity_contract":"distinct second node from the same enqueue ancestor reservation; bind before37506, do not alias first node3743A or clear the source ancestor at first publication",
        "ordering":"node link is shared before player store and payload resize/copy; native synchronization with readers is not established"
    });
    let transfer = json!({
        "caller_entry_rva":0x37f70,"flag_rva":0xb8aa1a,
        "flag_branches":"same flag returns AL0 at3804D; changed false returns AL1 at38046; changed true copies second range then releases original second nodes",
        "copy_call_rva":0x37fd1,"copy_return_rva":0x37fd6,"copy_entry_rva":0x36520,
        "copy_arguments":"RDX=first queue sentinel B8A9E8; R8 points to caller stack holding second head [B8AA00]; R9 points to caller stack holding second sentinel B8AA00; RCX also first sentinel but not used by the checked copy body",
        "source_load_rva":0x36560,"source_identity":"RBX=[R15], R15=entry R8; this is the source second node before allocation",
        "node_allocator_call_rva":0x36578,"node_allocator_return_rva":0x3657d,
        "node_allocator_rva":0x994970,"node_allocation_size":0x38,
        "bind_identity":"at3657D normal return RAX=new first node, RBX=source second node, RBP=destination sentinel; transfer source epoch into a distinct child reservation",
        "payload_allocator_call_rva":0x365b2,"payload_allocator_return_rva":0x365b7,
        "payload_allocator_limit":"same allocator, but result is serialized byte storage, not a node reservation",
        "source_scalar_copy_rvas":[0x36589,0x3658f],"source_payload":"[RBX+18..+20] copied to separate storage; capacity pointer derived from length and new buffer",
        "payload_copy_call_rva":0x365de,"payload_copy_import_slot_rva":0x9b7600,
        "local_link_stores":[0x365eb,0x365f2],
        "first_shared_link_store_rva":0x365fa,"tail_store_rva":0x365fd,
        "source_iterator_advance_rva":0x36607,
        "source_node_free_call_rva":0x3801f,"source_payload_free_call_rva":0x3800a,
        "identity_contract":"copy completes before publication in this body; original second node remains a separate live obligation until its own free. Equal payload/type/player does not identify the copy",
        "partial_failure":"SEH before normal copy return can leave some newly published copies and original sources live; neither whole-range success nor cancellation is inferred from the exception"
    });
    let replay = json!({
        "adapter_rva":0x590730,"adapter_tail_rva":0x590733,"entry_rva":0x37690,
        "input_stream":"adapter moves RDX into RCX; entry RCX retained in R14; no World/session token is an argument here",
        "version_reader_call_rva":0x376d7,"version_commit_call_rva":0x376e0,
        "version_guard_rva":0x376e3,"reject_target_rva":0x376e9,"maximum_version":1,
        "count_reader_call_rva":0x37754,"count_commit_call_rva":0x3775d,
        "count_discriminator_rva":0x37763,"explicit_count_target_rva":0x37952,
        "measure_remaining_call_rva":0x3776f,
        "node_allocations":[
            {"branch":"count == -1, remaining-byte loop","call_rva":0x377a5,"return_rva":0x377aa,
                "first_shared_store_rva":0x377fb,"tail_store_rva":0x377fe,
                "player_reader_rva":0x37814,"reserved_scalar_reader_rva":0x37854,
                "payload_length_reader_rva":0x37894,"payload_ingest_rva":0x3792c},
            {"branch":"explicit nonzero count, decremented loop","call_rva":0x3797b,"return_rva":0x37980,
                "first_shared_store_rva":0x379d1,"tail_store_rva":0x379d4,
                "player_reader_rva":0x379ea,"reserved_scalar_reader_rva":0x37a2a,
                "payload_length_reader_rva":0x37a6a,"payload_ingest_rva":0x37b02}
        ],
        "destination":"second queue B8AA00/B8AA08; allocation size38, record player=node+10, extra scalar=node+14, byte buffer atnode+18",
        "ordering":"both branches publish zero-initialized byte-buffer fields before reading the player/extra scalar/length/payload from the incoming stream",
        "normal_return":"version rejection returns AL0; supported empty or completed ingestion reaches AL1 at37B19 and flag B8AA1C=1 at37B3B; neither return establishes World/session provenance",
        "not_direct_execute":"complete37690 body contains stream virtual calls and node publication, no direct call to37B50; recorded second nodes can be copied by37F70/36520 and later consumed by372F0/37B50",
        "reservation_contract":"reserve serialized-ingestion ancestry at37690 entry, bind each normal node allocation at377AA/37980 before shared store; source epoch must come from actual session/stream authority, never the current execution generation"
    });
    let cleanup = json!({
        "entry_rva":0x380b0,"first_prototype_receiver_rva":0x380d2,
        "first_prototype_delete_call_rva":0x380e3,"delete_slot":0,"delete_edx":1,
        "registry_tree_release_call_rva":0x3810b,"registry_node_free_call_rva":0x38124,
        "registry_root_clear_rva":0x3814f,
        "first_queue":{"head_rva":0xb8a9e8,"node_snapshot_rva":0x38170,
            "payload_free_call_rva":0x3818a,"node_free_call_rva":0x3819f,
            "node_free_return_rva":0x381a4,"sentinel_reset_rvas":[0x381b0,0x381b7]},
        "second_queue":{"head_rva":0xb8aa00,"node_snapshot_rva":0x381d0,
            "payload_free_call_rva":0x381ea,"node_free_call_rva":0x381ff,
            "node_free_return_rva":0x38204,"sentinel_reset_rvas":[0x3820e,0x38215]},
        "other_free_paths":[
            {"entry_rva":0x37f70,"purpose":"original second nodes after copy", "payload_call_rva":0x3800a,"node_call_rva":0x3801f,"node_return_rva":0x38024},
            {"entry_rva":0x372f0,"purpose":"first node after normal execution", "payload_call_rva":0x37388,"node_call_rva":0x3739d,"node_return_rva":0x373a2}
        ],
        "free_receiver":"for all listed queue frees RDX=record/payload, R8=size; node comes from RDI in cleanup/37F70 and RBX in372F0; 36E20 moves RDX to RCX and tailcalls9908F0",
        "free_leaf_rva":0x36e20,"free_leaf_hex":"488bcae9c89a9500","native_free_target_rva":0x9908f0,
        "flags":"38221 B8AA19=0;38228 B8AA1A=0;3822F B60C18=1;38236 B8AA1B=0;3823D B8AA1C=0; these are original stores, not a producer seal",
        "pre_free_contract":"validate opaque node identity and set a cancellation-in-progress reservation before native free; retire only on normal return while preserving original return/SEH; SEH is unresolved because partial destruction cannot be undone",
        "first_destruction_limit":"380E3 deletes registered execution instances before queue traversal. Queue cancellation alone does not drain running37B50/prototype consumers; receiver and synchronous lifetimes must be safe before this first virtual delete",
        "no_epoch_inference":"free/flags0/registry root0 do not prove all network/session producers stopped, no pending publication, or no new allocation at a reused node address"
    });
    Ok(
        json!({"schema":"native-operation-secondary-queue-static-v1",
        "frames":entries,"computed_references":refs,
        "enqueue_second":enqueue_second,"transfer_copy":transfer,
        "serialized_ingestion":replay,"cleanup":cleanup,
        "not_established_by_this_audit":[
            "runtime identities/serialization, source-to-copy token binding, cancellation/ABA and SEH settlement",
            "incoming network/session/World epoch for serialized replay or delayed packets",
            "executed free target behavior or guarantee that an exceptional free left storage alive",
            "all publication/free consumers, native producer seal, full-coverage shutdown or product acceptance"
        ],"certificate_issued":false}),
    )
}
