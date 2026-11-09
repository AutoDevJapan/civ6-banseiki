//! Terminal native removal is distinct from transport disconnect.
#[path = "terminal_fixed.rs"]
pub(super) mod fixed;
#[path = "terminal_flow.rs"]
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
    let shared = [0x7f9220, 0x836a90, 0x81bd80, 0x836940, 0x845be0]
        .into_iter()
        .map(|entry| {
            let spec = lease::fixed::FRAMES
                .iter()
                .find(|f| f.begin == entry)
                .ok_or("missing terminal shared frame")?;
            complete_frame(host, spec)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(json!({"evidence":evidence,"shared_complete_frames":shared,
"remove":{"entry":0x7f9220,"arguments":"RCX=registry,EDX=native ID","session_load":0x7f9287,"destructor_call":0x7f9298,"destructor_return":0x7f929a,"destructor_arguments":"RCX=[node+8]=session,EDX=1,slot0","slot_clear":0x7f929a,"node_unlink":[0x7f92c7,0x7f92e1],"node_free_call":0x7f92f2,"node_free":0xa51f90,"node_size":0x18,"normal_return":0x7f9309},
"typed_session":{"tables":[0x1043088,0x10436e0,0x1045bd8],"destructors":[0x836a90,0x81bd80,0x836a90],"shared_body":0x836940,"resource_destructor":0x845be0,"direct_transport_close":0x845bfe,"direct_transport_close_slot":0x40,"callback_destructor":0x845c40,"callback_clear":0x845c42,"limit":"the direct845BE0 close path does not call845D30; already closed transport is not another join success"},
"registry":{"existing_full_entry":0x7f8a70,"body_call":0x7f8a7f,"body":0x7f8310,"body_full_end":0x7f8a62,"first_session_destructor":0x7f8373,"first_slot_clear":0x7f8375,"node_free":0x7f84d1,"normal_wrapper_return":0x7f8aa3,"contract":"reserve all known registry/session incarnations before the first destructor; retain key/address/callback tombstones through the existing77 wrapper full normalreturn including optional registry free"},
"adapter_contract":{"new_full_entries":[0x7f9220,0x836a90,0x81bd80],"remove_first8":"48895c2408488974","session_first8":"48895c2408574883","stolen_lengths":[5],"typed_destructor_return_is_terminal":false,"contract":"tombstone at exact native remove or registry root; inherit target by same native node and typed destructor caller7F929A/7F8375; require native join only for still-live transport; typed normalreturn records destruction while only root fullreturn permits SessionClosed; SEH retains unresolved tombstone"},
"certificate_issued":false,"not_established_by_this_extension":["runtime installation and live execution","all upstream native input epochs and natural producer seal","all-domain normal-runtime Rebuild authority"]}))
}
