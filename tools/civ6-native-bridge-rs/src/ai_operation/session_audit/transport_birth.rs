//! Session-to-transport and pre-start network/thread native birth evidence.
#[path = "transport_birth_fixed.rs"]
pub(super) mod fixed;
#[path = "transport_birth_flow.rs"]
pub(super) mod flow;

use super::{complete_frame, inspect_image, lease, pe::Pe, Value};
use serde_json::json;

pub(super) fn inspect(host: &Pe<'_>) -> Result<Value, String> {
    let evidence = inspect_image(
        host,
        fixed::FRAMES,
        flow::BLOCKS,
        flow::REFERENCES,
        flow::SLOTS,
        false,
    )?;
    // Reuse the already fixed whole bodies rather than maintaining a second
    // specimen for factories, constructors and local stop/join functions.
    let shared = [
        0x81b330, 0x81c680, 0x83ade0, 0x84dea0, 0x850c00, 0x823e00, 0x83fc10, 0x83f800, 0x860b10,
        0x84e4e0, 0x857360, 0x98c070, 0x98bef0,
    ]
    .into_iter()
    .map(|entry| {
        let spec = lease::fixed::FRAMES
            .iter()
            .find(|f| f.begin == entry)
            .ok_or("missing shared transport complete frame")?;
        complete_frame(host, spec)
    })
    .collect::<Result<Vec<_>, String>>()?;
    for (cell, name) in [
        (0xf977a8, "CreateThread"),
        (0xf97798, "ResetEvent"),
        (0xf977a0, "CreateEventA"),
        (0xf97750, "WaitForSingleObject"),
    ] {
        host.import(cell, "KERNEL32.dll", name)?;
    }
    let factory = json!({
            "slot":0x408,"arguments":"RCX=already registered session, EDX=transport mode; the fixed factories pass [session+30] as constructor RDX",
            "variants":[
                {"session_table":0x1043088,"factory":0x81b330,"constructor":0x850c00,"transport_table":0x10482c8},
                {"session_table":0x10436e0,"factory":0x81c680,"constructor":0x823e00,"transport_table":0x1044478},
                {"session_table":0x1045bd8,"factory":0x83ade0,"constructor":0x84dea0,"transport_table":0x1047e60}],
            "normal_result":"original RAX=transport, or null allocation result; the constructors return through the factory's original caller because the fixed factories tail them",
            "before_session_publication":[
                {"factory_call":0x839058,"factory_return":0x83905e,"session_register":"R12","store":0x83906b},
                {"factory_call":0x83ef3e,"factory_return":0x83ef44,"session_register":"R14","store":0x83ef53},
                {"factory_call":0x83fc5d,"factory_return":0x83fc63,"session_register":"RBX","store":0x83fc72}],
            "session_field":0x28,"callback_field":0x30,
            "transport_callback_store":0x84deb1,"transport_callback_field":8,
            "native_lease_contract":"reserve from an existing session birth lease at factory entry; on normal RAX bind a distinct transport incarnation and verify callback identity before the original Session+28 store; never use current World or raw callback numeric ID to mint a lease",
            "constructor_limit":"derived850C00 registers external callbacks before its return; this boundary proves before Session+28 publication, not before every possible external callback. An active factory reservation must remain available for earlier constructor-side effects",
            "replacement":{"entry":0x83fc10,"old_close":0x83fc4d,"store":0x83fc72,
                "other_old_close_calls":[0x839049,0x83eefb],"initial_null_store":0x83f823,
                "scope":"normal old845D30 return precedes replacement factory and new Session+28 store; transport close is not the terminal lifetime of the persistent registry session"}
    });
    let network = json!({
            "local_entry":0x84e4e0,"arguments":"RCX=transport, DX=port",
            "old_network_close_slot":0x58,"old_network_close_call":0x84e4f3,
            "mode_test":0x84e4f6,"required_mode":1,
            "constructor_call":0x84e534,"constructor":0x860b10,"constructor_return":0x84e539,
            "constructor_arguments":"RCX=new heap network, EDX=port","table":0x1049808,
            "transport_field":0x10,"publish":0x84e539,
            "start_call":0x84e54c,"start_entry":0x857360,"threaded_flag":0x45,
            "contract":"own the84E4E0 invocation for an already bound transport; on860B10 normal RAX under caller84E539 bind that exact network before the pointer store and possible thread start; null/failure/unwind retains no successful birth",
            "alternatives":[
                {"transport_table":0x10482c8,"entry":0x852240,"mode":2,"constructor":0x861c10,"publish":0x8522a1,"start_call":0x8522b3,"fallback":0x84e4e0},
                {"transport_table":0x1044478,"entry":0x825900,"mode":4,"constructor":0x81f540,"publish":0x825960,"start_call":0x825972,"fallback":0x84e4e0}],
            "alternative_limit":"record the different constructors and transport classes; this local network1049808 slice does not assign their queue or stop/join semantics"
    });
    let thread = json!({
            "start_wrapper":0x857360,"arguments":"RCX=existing bound network",
            "network_name_query":0x85737d,"saved_functor":"{network,8570B0} on original stack",
            "creator_call":0x857396,"creator":0x98c070,
            "creator_arguments":"RCX=&functor,RDX=0,R8=native name,R9d=network+74,stackarg5=0",
            "creator_return":0x85739b,"network_field":0xc0,"network_publish":0x85739b,
            "constructor_call":0x98c0cc,"constructor":0x98bef0,"constructor_return":0x98c0d1,
            "constructor_result":"RAX=new thread object with table106D030; no thread has been created by this constructor body",
            "start_vcall":0x98c0f3,"start_slot":8,"start_entry":0x98c170,
            "saved_owner_store":0x98c1e9,"saved_callback_store":0x98c1e0,
            "native_create_call":0x98c2b0,"native_create_iat":0xf977a8,
            "native_create_api":"KERNEL32.dll!CreateThread","native_create_arguments":"RCX=0,RDX=stack size,R8=98C570 or98C590,R9=thread object,stackarg5=flags,stackarg6=&thread+C8",
            "handle_publish":0x98c2b6,"normal_start_success":0x98c2c2,"normal_start_failure":0x98c2ec,
            "failure_cleanup":"98C070 tests original start AL; failure deletes the new thread and returns null rather than a live thread pointer",
            "before_start_contract":"use98BEF0 normal return98C0D1 under active98C070 and857360 with the saved network/session/transport token; bind original RAX before98C0F3 starts it. Binding only at outer85739B is after the thread may already run",
            "native_entry":0x98c570,"callback_loop":0x98c4a0,
            "callback_call":0x98c500,"callback_arguments":"RCX=[thread+8]=network,RDX=[thread+A0]; callback [thread+10]=8570B0 tail-calls network virtual+10",
            "local_network_execute_slot":0x10,"local_network_execute":0x861550,
            "stop_contract":"retain the exact local transport/network/thread incarnation through existing845D30/84EC00/8573B0/98C650 normal AL1 proof and full receive-frame drainage; null pointers, callback return or outer wrapper return do not substitute"
    });
    let mut native_events = json!({
            "receiver":0x861550,"receiver_arguments":"RCX=bound native network",
            "poll":0xc66350,"poll_call":0x86159b,"poll_return":0x8615a0,
            "poll_arguments":"RCX=network+D0 native host,RDX=original rsp+30 event output,R8d=0",
            "poll_outcome":"positive original EAX enters event construction; zero/negative exits the loop",
            "source_packet":"event output+18, copied by the caller into RBX; packet+10=bytes,+18=count,+8=flags",
            "construct":0x856e80,"construct_call":0x8615e4,"event_size":0x40,
            "constructor_copy":0x856ee3,"copy_target":0x982b00,
            "enqueue":0x8570c0,"enqueue_call":0x8615f1,
            "source_packet_release":0x8615fe,"packet_release":0xc65a60
    });
    let queue = json!({
            "queue_layout":{"vectors":0x10,"vector_stride":0x20,"index":0x70,"lock":0x80,"element_stride":0x40},
            "append":0x857470,"append_call":0x8570ef,"append_copy_call":0x8574a6,"append_copy":0x509a00,
            "append_copy_result":"RAX=destination byte-vector subobject at event+8; independent bytes are allocated and copied, source is retained",
            "growth":0x857160,"growth_scope":"all growth branches copy event scalars and byte vectors, then release old vectors/storage; raw element addresses cannot be permanent packet identity",
            "extract":0x8573f0,"extract_call":0x84edeb,"extract_copy":0x856df0,
            "extract_copy_call":0x85741f,"range_copy":0x81f450,"range_copy_call":0x856e5d,
            "extract_result":"RAX=output vector; every40h event and its byte vector is copied into separate storage, producer index toggles and the next producer vector is cleared",
            "consumer":0x84edb0,"event_kind_load":0x84ee23,"data_event_kind":4,
            "transport_callback_call":0x84ee75,"transport_callback_slot":0x68,"transport_callback":0x84e9e0,
            "connection":"[transport+8] callback","connection_slot":0x28,"incoming":0x860000,
            "destroy_leaf":0x857040,"destroy_limit":"release event+8 vector using A51F90; not a seal or native join",
            "packet_contract":"capture a once-issued packet receipt at the bound network's original poll/admission before enqueue; propagate it through event construction, deep-copy append/growth/extraction and the transport callback;860000 consumes the inherited receipt, never the current World after a delay",
            "frontier_limit":"native poll/Host queue birth is observable here, but the OS/kernel buffer arrival, remote send epoch and all alternateSteam/kind4 receive queues are not certified. Data already pending before observation remains unknown rather than receiving a fresh epoch"
    });
    native_events
        .as_object_mut()
        .unwrap()
        .extend(queue.as_object().unwrap().clone());
    let adapter = json!({
            "session_factory":{"entries":[0x81b330,0x81c680,0x83ade0],"first_8_hex":"40534883ec30488b","stolen_lengths":[2,4],"original_returns":[0x83905e,0x83ef44,0x83fc63]},
            "network_ctor":{"entry":0x860b10,"first_8_hex":"48895c2410488974","stolen_lengths":[5],"continue":0x860b15,"original_return":0x84e539},
            "network_start":{"entry":0x857360,"first_8_hex":"40534883ec40488d","stolen_lengths":[2,4],"continue":0x857366},
            "thread_creator":{"entry":0x98c070,"first_8_hex":"48895c240848896c","stolen_lengths":[5],"continue":0x98c075,"original_return":0x85739b},
            "thread_ctor":{"entry":0x98bef0,"first_8_hex":"48895c2408574883","stolen_lengths":[5],"continue":0x98bef5,"original_return":0x98c0d1},
            "separate_lifetimes":"registry session birth/destroy, transport disconnect/replacement, network and native thread births have separate incarnations.845D30 must not terminate the persistent session lease merely because one transport closes",
            "abi":"whole original normal/SEH invocation, original GPR/XMM/flags/LastError, stack arguments and AL/null/failure outcomes preserved; no mid-instruction or arbitrary body patch"
    });
    Ok(json!({
        "evidence":evidence,"shared_whole_frames":shared,
        "factory":factory,"network":network,"thread":thread,
        "native_events_before_callback":native_events,"adapter_contract":adapter,
        "certificate_issued":false,
        "not_established_by_this_extension":["runtime hook installation or native execution","all alternate transports and OS/native queue ancestors","all-domain producer seal or normal-runtime Rebuild authority"]
    }))
}
