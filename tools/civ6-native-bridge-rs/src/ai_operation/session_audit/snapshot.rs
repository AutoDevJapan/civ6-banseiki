//! Accepted snapshot stream ownership and deferred-message byte queue evidence.
#[path = "snapshot_fixed.rs"]
pub(super) mod fixed;
#[path = "snapshot_flow.rs"]
pub(super) mod flow;

use super::{exact, inspect_image, pe::Pe, Value};
use serde_json::json;

/// Compare independently encoded producer and consumer receiver fields.
pub(super) fn queue_layout(host: &Pe<'_>) -> Result<Value, String> {
    let field = u32::from_le_bytes(host.span(0x842574, 4, true)?.try_into().unwrap());
    let consumer = u32::from_le_bytes(host.span(0x83d7d2, 4, true)?.try_into().unwrap());
    let pointers = u32::from_le_bytes(host.span(0x83d82b, 4, true)?.try_into().unwrap());
    let capacity = u32::from_le_bytes(host.span(0x83d821, 4, true)?.try_into().unwrap());
    let head = u32::from_le_bytes(host.span(0x83d7f1, 4, true)?.try_into().unwrap());
    let count = u32::from_le_bytes(host.span(0x83d858, 4, true)?.try_into().unwrap());
    if field != consumer
        || field.checked_add(8) != Some(pointers)
        || field.checked_add(0x10) != Some(capacity)
        || field.checked_add(0x18) != Some(head)
        || field.checked_add(0x20) != Some(count)
    {
        return Err("deferred queue producer/consumer receiver identity".into());
    }
    Ok(
        json!({"session_queue":field,"pointer_table":pointers,"capacity":capacity,
        "head":head,"count":count,"record_size":0x30,
        "producer_index":"(head+count)&(capacity-1)",
        "consumer_index":"head&(capacity-1)",
        "ordering_scope":"this queue appends at the tail and removes at the head; no cross-queue, worker, network or map-command barrier follows from that fact"}),
    )
}

pub(super) fn inspect(host: &Pe<'_>) -> Result<Value, String> {
    let evidence = inspect_image(
        host,
        fixed::FRAMES,
        flow::BLOCKS,
        flow::REFERENCES,
        flow::SLOTS,
        false,
    )?;
    // These complete bounded leaves have no pdata entry.
    for (begin, end) in [(0x168b90, 0x168bbe), (0x16d280, 0x16d2ac)] {
        if host
            .functions
            .iter()
            .any(|f| f.begin < end && f.end > begin)
        {
            return Err(format!(
                "snapshot leaf unexpectedly covered by pdata at {begin:#x}"
            ));
        }
    }
    exact(
        host,
        0x16d280,
        "4885d27516488b158cba16028b0d8eba16024885d275048d4205c383f904488bca0f84c9190000e9d4070000",
    )?;
    let descriptor = host.span(0xfd3cc0, 16, false)?;
    if descriptor
        != [2_u32, 0, 4, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
    {
        return Err("snapshot descriptor kind/transport retention".into());
    }
    let layout = queue_layout(host)?;
    Ok(json!({
        "evidence":evidence,
        "snapshot_admission":{
            "entry":0x273f40,"owner_table":0xfd35c8,"slot":0xe8,
            "arguments":"RCX=registered native owner, RDX=bytes, R8d=signed byte count, R9d=flags",
            "receiver_requirement":"capture the existing owner+8 session incarnation at entry, before the original body replaces RCX; this field is a native session pointer, not a World generation",
            "busy_call":0x273f73,"busy_predicate":0x168b90,"busy_leaf_end":0x168bbe,
            "busy_flow":"AL1 if global22D8D09 is set and request kind22D8D10 equals2, or engine.currentView5400 equals engine.LoadGameView14F0; otherwise AL0",
            "branch":0x273f7a,"accept_path":0x274149,
            "busy_outcome":"nonzero AL follows the logging path and jumps2741D5; it does not call16C920 or replace the owned stream",
            "source_stream":{"constructor":0x274180,"byte_copy_call":0x27418f,
                "address":"original rbp-60","destroy_call":0x2741d0},
            "descriptor_rva":0xfd3cc0,"descriptor_words":[2,0,4,0],
            "request_call":0x2741be,"request_entry":0x16c920,
            "request_arguments":"RCX=0,RDX=descriptor,R8=local source stream",
            "request_wrapper":"16C920 passes source stream as stack argument5, R8=descriptor, EDX=2, R9=0 to16B5C0",
            "request_return":0x16c938,
            "outcome_limit":"273F40 does not retain or test the request AL before destroying its temporary stream; its own return is not an accepted-request receipt"
        },
        "accepted_request":{
            "entry":0x16b5c0,
            "arguments":"RCX=optional path, EDX=request kind, R8=descriptor, R9=optional object, stack+28=source stream",
            "busy_global":0x22d8d10,"busy_branch":0x16b5f5,"busy_reject":0x16bd74,
            "busy_result":"AL0, without entering the acceptance state/copy path",
            "validation_reject":0x16b7df,"validation":"for descriptor+8==4 and kind2, null source stream rejects; other request kinds have distinct checks",
            "kind_publish":0x16b8e4,"descriptor_publish":0x16b9df,
            "stream_copy_call":0x16bd4c,"stream_copy_entry":0x166b40,
            "possible_producer_call":0x16bd68,"possible_producer":0x163cc0,
            "producer_guard":"engine.currentView==engine+14F8 skips163CC0; otherwise it is called after owned stream copy and before success AL1",
            "success_set":0x16bd6d,
            "runtime_requirement":"reserve an opaque accepted-request candidate before original16B5C0; bind its owned stream and record producer use inside that full invocation; commit normal AL1 separately from partial writes or SEH; a callback issued before AL1 cannot be assigned authority from a future current World",
            "concurrency_limit":"the pinned busy test and kind store are separate non-atomic instructions; they are not a native CAS or a proven concurrent-load serialization guarantee"
        },
        "owned_stream":{
            "replace_entry":0x166b40,"arguments":"RCX=source stream, EDX=descriptor first word",
            "global":0x22d8d18,"kind_global":0x22d8d20,
            "old_destroy_call":0x166b74,"clear":0x166b81,
            "heap_allocation":0x166bb6,"heap_size":0x28,"stream_table":0xfc1798,
            "object_publish":0x166bda,
            "source_size_call":0x166be7,"source_reset_call":0x166c05,
            "copy_call":0x166c4b,"copy_arguments":"RCX=source,RDX=read descriptor,R8=owned stream+8,R9=0; source virtual+28 transfers the byte range into independently allocated storage",
            "owned_reset_call":0x166c5a,"kind_publish":0x166c5d,
            "publication_limit":"the heap object is stored globally before its byte copy and reset; object pointer publication alone is not successful copy completion",
            "identity_requirement":"retain request incarnation, source stream, exact destination heap object and its normal copy outcome; old destruction or allocator reuse cannot attach a previous receipt to a new stream",
            "selection_entry":0x16d280,"selection_leaf_end":0x16d2ac,
            "selection_arguments":"ECX=kind,RDX=optional stream; null RDX selects globals22D8D18/22D8D20; missing stream returns EAX5",
            "kind4_tail":0x16ec70,"other_kind_tail":0x16da80,
            "consumer_limit":"16D280 only selects and tail-dispatches the stream. Its return must be preserved as the original outcome; it does not itself free the global, prove Context birth or transfer a new World epoch"
        },
        "deferred_queue":{
            "layout":layout,"producer_full_frame":0x842030,"entry_session":"RCX retained in R14","entry_message":"RDX retained in R15",
            "queue_gate_call":0x842449,"queue_gate_target":0x83fbc0,
            "queue_gate":"gate AL!=0 or existing pending count with inactive pump selects the queue branch; this is not a first-receive test",
            "serialize_call":0x8424a6,"serialize":0x7fb1b0,
            "source_record":"original rbp-70 vector; scalar+20 from message+10, scalar+24 from message+24, flag+28 from saved entry flag",
            "enqueue_call":0x84257c,"enqueue":0x847510,
            "enqueue_arguments":"RCX=session+D8,RDX=source record",
            "slot_allocation_call":0x847571,"move_call":0x8475ad,"move_entry":0x527440,
            "move_arguments":"RCX=destination heap30h slot,RDX=source record",
            "move_normal_return":0x8475b2,"move_result":"RAX=destination slot; move three vector pointers and zero the source; no byte-vector clone here",
            "scalar_copy":[0x8475b2,0x8475b8,0x8475be],"count_publish":0x8475c5,
            "growth":0x846ef0,"identity":"heap slot plus queue/session and enqueue incarnation; pointer-table position changes on growth, and heap slots are reused",
            "enqueue_result_limit":"847510 does not promise its final RAX is the slot; bind the527440 normal RAX before caller8475B2 resumes",
            "pump":0x83d570,"head_record":"RDI at83D7F5",
            "decode_call":0x83d819,"decode":0x7f9c30,"decoded_message":"RAX retained in RSI",
            "byte_free":0x83d850,"count_remove":0x83d855,"dispatch_call":0x83d882,
            "preserve_requirement":"acquire the existing head-slot receipt before decode; bind the decoded message on normal return before bytes are freed and head is advanced; decode failure/SEH must not mint current World authority",
            "cancel_entry":0x847390,"cancel_byte_free":0x8473e3,
            "cancel_limit":"storage clear destroys queued vectors and slots; it does not by itself seal the producer or join transport threads"
        },
        "input_frontier":{
            "first_observable_connection_callback":0x860000,
            "arguments":"RCX=connection callback,EDX=source ID,R8=bytes,R9d=byte count,stack+28=flag",
            "lookup_call":0x860021,"decode_call":0x860073,"dispatch_call":0x860136,
            "existing_provenance":"connection constructor85F9A0/native registry birth must already identify this connection/session incarnation before860021; capture a packet receipt once at this callback's input and retain it into its message(s)",
            "deferred_frontier":0x847510,
            "distinction":"D8..F8 queue records are serialization of already-decoded messages; they inherit the original message receipt and are not newly received packets when83D570 decodes them again",
            "ordering_limit":"no OS-buffer birth, earlier transport-queue admission, cross-thread FIFO or map-command/order barrier is established by the intra-queue arithmetic"
        },
        "adapter_contract":{
            "snapshot":{"entry":0x273f40,"first_8_hex":"48895c2408488974","stolen_lengths":[5],"continuation":0x273f45},
            "request":{"entry":0x16b5c0,"first_8_hex":"4c894c2420555341","stolen_lengths":[5],"continuation":0x16b5c5,
                "action":"retain source stack argument5 before entry, reserve before original code, preserve AL on every normal return, keep unresolved writes on original unwind"},
            "stream_copy":{"entry":0x166b40,"first_8_hex":"405556574157488d","stolen_lengths":[2,1,1,2],"continuation":0x166b46,
                "original_caller":0x16bd51,"action":"own the entire original copy and retain exact destination object independently of caller AL"},
            "enqueue":{"entry":0x847510,"first_8_hex":"48895c2408488974","stolen_lengths":[5],"continuation":0x847515,
                "original_caller":0x842581,"action":"inherit active842030 original message receipt before publication"},
            "move":{"entry":0x527440,"first_8_hex":"48895c2408574883","stolen_lengths":[5],"continuation":0x527445,
                "original_caller":0x8475b2,"action":"on normal original RAX bind the destination slot before scalar stores and count publication; unknown callers stay transparent"},
            "pump":{"entry":0x83d570,"first_8_hex":"4055535657415441","stolen_lengths":[2,1,1,1],"continuation":0x83d575,
                "action":"retain head-record lease before original decoder; transfer into decoder result under the matching pump invocation; a fresh current-World lookup never supplies provenance"},
            "abi":"whole original frame normal/SEH ownership; preserve original inputs, outputs, stack arguments, GPR/XMM, flags, LastError and alignment; SAVE_NONVOL and chained unwind must match stolen instructions; no arbitrary body patch or success receipt on unwind"
        },
        "not_established_by_this_extension":[
            "runtime packet/stream/queue receipts or their actual installation",
            "accepted request to cmd36 consumer and map invocation to new Context birth (separate runtime boundary)",
            "earlier native transport or OS receive admission before callback860000",
            "all alternate incoming, clone, discard and cancellation domains, or normal-runtime Rebuild authority"],
        "certificate_issued":false
    }))
}
