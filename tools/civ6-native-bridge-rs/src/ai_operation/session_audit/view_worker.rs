//! Existing View request incarnations carried through Host scheduler tasks.
#[path = "view_worker_fixed.rs"]
pub(super) mod fixed;
#[path = "view_worker_flow.rs"]
pub(super) mod flow;

use super::{inspect_image, pe::Pe, Value};
use serde_json::json;

/// Decode independent receiver displacements before asserting their relation.
pub(super) fn layout(host: &Pe<'_>) -> Result<Value, String> {
    let state = u32::from_le_bytes(host.span(0x8313b, 4, true)?.try_into().unwrap());
    let payload = u32::from(host.span(0x98617a, 1, true)?[0]);
    let set = u32::from(host.span(0x98616a, 1, true)?[0]);
    let parent = u32::from(host.span(0x985b5c, 1, true)?[0]);
    let owner = u32::from(host.span(0x83456, 1, true)?[0]);
    let callback = u32::from(host.span(0x8345a, 1, true)?[0]);
    let active = u32::from(host.span(0x985b8d, 1, true)?[0]);
    if payload.checked_add(parent) != Some(0x30)
        || set.checked_add(8) != Some(active)
        || payload.checked_add(0x48) != Some(owner)
        || payload.checked_add(0x50) != Some(callback)
        || state != 0x240
    {
        return Err("View task payload/state/set/callback receiver relation".into());
    }
    Ok(
        json!({"state_in_view":state,"payload_in_view":state+payload,
        "set_in_view":state+set,"active_group_in_view":state+active,
        "saved_owner_in_view":state+owner,"saved_callback_in_view":state+callback,
        "state_from_payload":parent,"owner_from_payload":0x48,"callback_from_payload":0x50}),
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
    for (begin, end) in [
        (0x985580, 0x9855b9),
        (0x83450, 0x8345b),
        (0x98caa0, 0x98caa9),
    ] {
        if host
            .functions
            .iter()
            .any(|f| f.begin < end && f.end > begin)
        {
            return Err(format!(
                "View worker bounded leaf has unexpected pdata at {begin:#x}"
            ));
        }
    }
    for (cell, name) in [
        (0xf977d0, "TlsGetValue"),
        (0xf97840, "CreateEventW"),
        (0xf97800, "WaitForSingleObjectEx"),
        (0xf97970, "CloseHandle"),
        (0xf97740, "SwitchToThread"),
    ] {
        host.import(cell, "KERNEL32.dll", name)?;
    }
    let layout = layout(host)?;
    Ok(json!({
        "evidence":evidence,"layout":layout,
        "request":{
            "view_constructor":0x82b00,"view_table":0xfad3f0,
            "birth_slot":8,"birth_entry":0x82e50,"finish_slot":0x10,"finish_entry":0x83160,
            "birth_arguments":"RCX=existing heap View; no command pointer or request identity is passed in the original ABI",
            "counter_increment":0x83095,"counter_field":0x2a8,
            "counter_limit":"a plain32-bit increment in the existing View; pointer/counter equality cannot establish a request incarnation or resolve a wrapped/reused worker",
            "worker_gate":0x83107,"worker_flag":0x2d8,"skip_worker_branch":0x8310e,
            "owner_store":0x8312a,"callback_store":0x83131,"callback":0x82da0,
            "start_call":0x8313f,"start_return":0x83144,
            "late_enable":{"entry":0x88d60,"flag_write":0x88d70,"active_field":0x2a0,
                "start_tail":0x88dac,"scope":"the same View can start a worker after request entry; inherit a saved request receipt, never the current View/World at this later callback"},
            "scope_requirement":"capture the accepted request at82E50 before original counter or callback changes; unresolved prior task/publication/worker frames prevent replacement or late adoption; carry that same request into late88D60 starts"
        },
        "payload_type":{
            "state":"S=View+240","payload":"P=S+8=View+248","set":"S+38=View+278",
            "state_base_constructor":0x985580,"base_state_table":0x106c3a0,
            "payload_table":0x106c390,"payload_table_store":0x985596,
            "parent_store":0x9855a9,"parent_value":"[P+28]=S",
            "state_table_override":0x82b6b,"state_table":0xfad3e0,
            "payload_execute_slot":8,"payload_execute":0x985b50,
            "state_execute_slot":8,"state_execute":0x83450,
            "state_execute_body":"mov RAX,RCX; mov RCX,[RCX+50]; jmp[RAX+58]",
            "worker_arguments":"RCX=[S+50]=View, EDX=the original scheduler TLS return low32; from P these fields are +48=View and +50=82DA0",
            "kind_write":0x98615b,"kind_field_in_state":0x48,"kind_value":1,
            "kind_limit":"this field is set before task publication and atomically exchanged to0 after the callback returns; it is execution state, not request identity, a producer seal or a World epoch"
        },
        "publication":{
            "entry":0x986150,"arguments":"RCX=S; forwards RCX=P,RDX=S+38,R8=local16-byte metadata to9858B0",
            "construct_call":0x986186,"construct_entry":0x9858b0,
            "group_prepare":0x986190,"active_group_store":0x986235,
            "allocate_call":0x9858ff,"allocator":0xe3ccf0,"allocate_return":0x985904,"allocation_size":0x10,
            "task_table":0x106c418,"task_table_store":0x985917,"task_payload_store":0x98591a,
            "payload_task_store":0x98592b,"payload_set_store":0x98592f,
            "publish_tail":0x985962,"publisher":0xe3e8c0,
            "publisher_arguments":"RCX=active scheduler group,RDX=T,R8=T-8",
            "before_publication_contract":"use the existing E3CCF0 gateway normal-return callback at original caller985904 under an active986150 and its saved accepted-request receipt; capture original RAX=T and expected P/S/set/group before the caller writes T/P links or publishes; unknown allocators remain transparent",
            "ordering_limit":"986150 writes busy state before allocation; constructor/allocator failure or SEH cannot be converted into a successful birth or a joined prior worker"
        },
        "execution":{
            "task_execute":0x9862e0,"task_table_slot":0x106c420,
            "payload_load":0x9862e6,"payload_links_cleared":[0x9862ef,0x9862f7],
            "tls_query_call":0x986302,"tls_query":0x98caa0,"tls_index_global":0x2d0a240,
            "tls_api":"KERNEL32.dll!TlsGetValue","tls_iat":0xf977d0,
            "argument_transfer":0x98630b,"payload_vcall":0x986310,
            "payload_callback_call":0x985b60,"payload_callback_return":0x985b63,
            "state_callback_tail":0x83457,"worker":0x82da0,
            "state_clear":0x985b69,"state_clear_instruction":"XCHG dword[S+48],EAX after EAX=0",
            "worker_tick_call":0x82dd3,"worker_tick":0x83c80,
            "terminal_state_test":"unsigned(state238-10)<=2 exits the worker loop; other states call83C80 and loop",
            "worker_normal_exit_tail":0x82e45,"exit_helper":0x6fc40,
            "entry_requirement":"the existing9862E0 hook captures T/P and its saved task receipt before original code clears P+8/P+10;82DA0 may inherit only that receipt through985B50/83450, with exact saved owner and callback; pointer/tid/current counter lookup cannot create a receipt",
            "identity_limit":"EDX is the scheduler TLS return low32, not a native OS thread ID or accepted-request generation; the scheduler can execute work on different threads or while waiting"
        },
        "finish_and_join":{
            "finish":0x83160,"wait_call":0x83387,"wait_entry":0x985b80,"wait_arguments":"RCX=S=View+240",
            "active_group_load":0x985b8a,"active_group":"[S+40]=[View+280]",
            "null_group_branch":0x985b94,"null_group_exit":0x985bb8,
            "group_wait_call":0x985bab,"group_wait":0xe3eaf0,"group_wait_return":0x985bb0,
            "group_pointer_clear":0x985bb0,"finish_resume":0x8338c,
            "current_arena_compare":0xe3eb2a,"different_arena_branch":0xe3eb2e,
            "early_return_branches":[0xe3eb39,0xe3eb48],"early_return_epilogue":0xe3ee30,
            "foreign_arena_loop":{"pending_field":0xb0,"pending_test":0xe3ebdd,
                "final_count_test":0xe3edfc,"final_count_limit":2,"wait_owner_test":0xe3ee04,
                "wait_owner_field":0x180,"loop_or_yield":0xe3ee0d},
            "native_wait_api":"KERNEL32.dll!WaitForSingleObjectEx","native_wait_iat":0xf97800,
            "native_wait_call":0xe3edd0,"native_wait_timeout":"EDX=FFFFFFFF,R8=0",
            "api_limit":"the wait is on a scheduler-created event while joining an arena/group; its API result is ignored before the loop rechecks native pending fields. It is not thread98C650/AL1 join",
            "required_receipt":"saved exact request/task/P/set/group incarnations, a classified original native group-wait normal return on an admissible arena path, and every already-bound worker82DA0 plus its execute/payload full frame returned or unwound with the appropriate outcome; null group, busy0, ptr0 and arbitrary wrapper return do not independently prove drainage",
            "view_transition":{"entry":0x63f60,"old_finish_call":0x63fac,"old_finish_return":0x63faf,
                "pending_to_current_store":0x63fb6,"new_entry_call":0x63fca,"new_entry_return":0x63fcd,
                "scope":"normal old currentView.virtual10 returns before pendingView becomes current5400 and receives virtual8; an exception cannot supply the missing old-worker receipt"}
        },
        "cancel_and_reuse":{
            "task_destructor":0x9857c0,"task_delete_call":0x9857dd,
            "destructor_scope":"replace T.vft and conditionally free10h; no P callback, cancellation acknowledgement or group wait is performed by this body",
            "reuse_requirement":"a discarded T, old worker, prior request counter and shared group need their own saved incarnation/outcome. Reusing View/P/group addresses never adopts unresolved work into a new request",
            "unproved":"all scheduler discard/copy paths and exception outcomes are not established by this extension"
        },
        "adapter_contract":{
            "view_request":{"entry":0x82e50,"first_8_hex":"405541544156488d","stolen_lengths":[2,2,2],"continuation":0x82e56},
            "start":{"entry":0x986150,"first_8_hex":"4883ec38488d0569","stolen_lengths":[4,7],"continuation":0x98615b,
                "note":"the11-byte prefix includes RIP-relative LEA and needs correct relocation/unwind; use the established whole-function gateway mechanism"},
            "allocator":{"existing_entry":0xe3ccf0,"required_original_caller":0x985904,"normal_result":"T in original RAX before publication"},
            "execute":{"existing_entry":0x9862e0,"action":"cooperate with existing jobs entry/finish; do not install a second hook at this address"},
            "worker":{"entry":0x82da0,"first_8_hex":"48895c2408574883","stolen_lengths":[5],"continuation":0x82da5},
            "finish":{"entry":0x985b80,"first_8_hex":"48895c2408574883","stolen_lengths":[5],"continuation":0x985b85,
                "original_caller":0x8338c},
            "group_wait":{"existing_entry":0xe3eaf0,"original_caller":0x985bb0,"action":"cooperate with native wait receipt, preserve original outcome and reject current-arena early-return proof"},
            "abi":"preserve original GPR/XMM/flags/LastError, stack arguments and alignment; own entire original normal/SEH frames; SAVE_NONVOL and full chained unwind follow displaced instructions; no arbitrary body patch and no normal receipt on exception"
        },
        "not_established_by_this_extension":[
            "runtime carrier creation, installation or original native execution",
            "every pending scheduler task, clone or discard path",
            "all-domain producer seal or normal-runtime Rebuild authority"],
        "certificate_issued":false
    }))
}
