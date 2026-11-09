//! Native session identity before registry publication and transport close/join.
//! The report describes adapter obligations; it does not issue a World lease.
#[path = "lease_fixed.rs"]
pub(super) mod fixed;
#[path = "lease_flow.rs"]
pub(super) mod flow;

use super::{inspect_image, pe::Pe, Value};
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
    host.import(0xf97750, "KERNEL32.dll", "WaitForSingleObject")?;
    host.import(0xf979e8, "KERNEL32.dll", "GetLastError")?;
    Ok(json!({
        "evidence":evidence,
        "registration":{
            "entry":0x7f8cd0,"arguments":"RCX=registry, EDX=kind, R8=owner",
            "id_global":0x235b874,"registry_global":0x235b878,
            "factory_slot":0x18,"factory":0x7f8ab0,
            "factory_call":0x7f8cf2,"factory_return":0x7f8cf5,
            "factory_arguments":"RCX=registry, EDX=kind, R8d=ID, R9=owner",
            "factory_result":"RAX=session; null returns skip publication",
            "owner_publish":0x7f8d06,"owner_field":8,
            "registry_publish":0x7f8d6c,"registry_node_field":8,
            "existing_key_branch":0x7f8d3e,
            "return_value":"EAX=global235B874 ID, including null-factory path; counter increments at7F8D91 and writes at7F8D98 without an epoch check",
            "aba_requirement":"retain old slot value and distinct registry, session, callback and transport incarnations across replacement, ID wrap and address reuse; raw key/pointer equality is insufficient",
            "id_write":0x84a4f7,"session_id_field":8,
            "factory_selection":[
                {"kind":2,"constructor":0x81af30,"table":0x1043088},
                {"kind":4,"constructor":0x81bd40,"table":0x10436e0},
                {"kind_condition":"neither 2 nor 4","constructor":0x8367f0,"table":0x1045bd8}],
            "before_publication_bind":"wrap the entire original7F8AB0; only original caller7F8CF5 under an active7F8CD0 reservation may bind returned RAX before the caller resumes; retain saved factory inputs R8d and R9",
            "limit":"owner is an opaque native owner; neither owner nor native ID proves a World generation; a late install cannot synthesize birth for an already registered object"},
        "connection":{
            "constructor":0x85f9a0,"caller_return":0x83f86c,
            "table":0x10494a8,"id_field":8,"session_field":0x30,
            "session_publish":0x83f871,"incoming_slot":0x28,"incoming":0x860000,
            "lookup":0x7f9960,"lookup_arguments":"RCX=[global235B878], EDX=[connection+8]",
            "lookup_flow":"registry+10 buckets, +18 bucket count; node key+0, value+8, next+10; match returns value, missing returns zero",
            "identity_requirement":"connection address and incarnation must already match the saved registry/session/ID birth; raw numeric key lookup is not a generation check"},
        "dispatch":{
            "entry":0x842030,"arguments":"RCX=session, RDX=message, R8b=flag",
            "tables":[0x1043088,0x10436e0,0x1045bd8],"slot":0x238,
            "first_8_hex":"4055535657415441","stolen_lengths":[2,1,1,1],
            "continuation":0x842035,"frame_end":0x842c83,
            "frame_requirement":"seven nonvolatile pushes, dynamic8340 stack allocation and three pdata partitions; a gateway owns the complete original return or unwind",
            "continuation_slot":0x240,"continuation_entry":0x83dc80,
            "operation_edge":"message IDs below1000 tail through session+18 owner virtual+120; fixed owner FD35C8+120 is815FF0, then7FB0A0 calls message virtual+20 once unless message+34 is already set",
            "world_lookup_boundary":"the operation message265660 obtains the current Context only after this admission; propagate the inherited native lease and never mint a current epoch there"},
        "close":{
            "entry":0x845d30,"arguments":"RCX=session",
            "transport_field":0x28,"transport_close_call":0x845da7,
            "transport_slot":0x40,"transport_delete_call":0x845db9,
            "transport_clear":0x845dbb,
            "full_destroy_entry":0x845be0,"full_destroy_transport_close":0x845bfe,
            "callback_delete":0x845c40,"callback_field":0x30,
            "registry_remove":0x7f9220,"registry_remove_delete_call":0x7f9298,
            "session_destructor":0x836940,"deleting_destructor":0x836a90,
            "reconnect_entry":0x83fc10,"reconnect_old_close":0x83fc4d,
            "reconnect_publish":0x83fc72,
            "incarnation_requirement":"845D30 closes a transport while the registry session and callback survive; reopening the same address/ID requires a distinct native transport incarnation and independently retained World provenance"},
        "local_thread_join":{
            "transport_tables":[0x1047e60,0x10482c8],"close_slot":0x40,"close_entry":0x84ebd0,
            "close_sequence":"84EC50 releases queued native objects; virtual+18 receives DL1; virtual+58 tail-enters84EC00",
            "network_field":0x10,"join_wrapper":0x8573b0,"thread_field":0xc0,
            "thread_creator":0x98c070,"thread_constructor":0x98bef0,
            "thread_table":0x106d030,"join_slot":0x30,"join_entry":0x98c650,
            "join_call":0x8573c6,"join_return":0x8573c9,
            "join_arguments":"RCX=network thread; EDX=FFFFFFFF (INFINITE)",
            "wait_call":0x98c6b2,"wait_iat":0xf97750,"wait_api":"KERNEL32.dll!WaitForSingleObject",
            "wait_arguments":"RCX=[thread+B0], EDX=saved timeout",
            "success_branch":0x98c6e5,"success_flow":"clear thread+B0; return AL1 and clear thread+CD; other paths return AL0",
            "wait_result_test":"EAX & FFFFFF7F ==0 selects success (0 or80); timeout102, failureFFFFFFFF and other values do not",
            "important_limit":"8573B0 ignores AL before deleting the thread; original wrapper return alone is insufficient join evidence",
            "separate_transport":{"table":0x1044478,"close":0x826160,"join_slot_target":0x8261b0,
                "limit":"native4 transport uses81FF90/822FE0; same-local-thread join is not assigned to it"}},
        "adapter_contract":contract(),
        "not_established_by_this_extension":[
            "a World generation in registry/session/connection native fields or opaque owner",
            "World502E0 record callback+10 target that invokes same-session845D30 or845BE0 before380B0",
            "all transport incarnations, remote receive paths, queued copies and cancellation/join domains"],
        "certificate_issued":false
    }))
}

fn contract() -> Value {
    json!({
        "birth":{"entry":0x7f8ab0,"first_8_hex":"48895c2408488974",
            "stolen_lengths":[5],"continuation":0x7f8ab5,"original_caller":0x7f8cf5,
            "action":"bind RAX only on normal return under active7F8CD0; inherit previously proven World provenance from that reservation, otherwise remain unbound"},
        "admission":{"entry":0x860000,"first_8_hex":"48895c2420574156",
            "stolen_lengths":[5],"continuation":0x860005,
            "action":"acquire the already bound connection/transport incarnation before the global registry lookup and retain through every original normal/SEH exit"},
        "dispatch":{"entry":0x842030,"first_8_hex":"4055535657415441",
            "stolen_lengths":[2,1,1,1],"continuation":0x842035,
            "action":"lookup an inherited lease for both session and message; absence never falls back to the current World"},
        "stop":{"entry":0x845d30,"first_8_hex":"48895c2408574883",
            "stolen_lengths":[5],"continuation":0x845d35,
            "action":"close admission for the exact saved transport incarnation before original first use; retain the closing tombstone on SEH"},
        "join":{"entry":0x98c650,"first_8_hex":"48895c2410564883",
            "stolen_lengths":[5],"continuation":0x98c655,"original_caller":0x8573c9,
            "action":"capture original AL and identical thread/network/transport incarnation before original caller deletes the thread; only AL1 plus all original admission frames returned can contribute a join receipt"},
        "cleanup_requirement":"before original380B0/prototype delete380E3, require proven World-to-native-session binding, same-incarnation admission closure, successful native joins and full-frame producer drainage; World502E0 callback return alone grants none",
        "abi_requirement":"preserve original arguments, results, GPR/XMM, flags, LastError, stack arguments and alignment; register valid unwind for every displaced instruction and original full-frame ownership; propagate original SEH without normal-return receipts",
        "identity":"registry lifetime + nativeID + session incarnation + connection incarnation + transport incarnation + native thread identity; retain separate proven World binding",
        "runtime_hooks_installed_by_this_extension":false
    })
}
