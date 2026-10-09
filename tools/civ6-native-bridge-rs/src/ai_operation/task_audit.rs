//! Fixed-file evidence for the real JobManager path; no handles or invocation.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const HOST_SHA: &str = "e7450823cc8e00468cff7b9d7b97c63140eae38ae1d774ba4efa437556c42d63";

fn bytes(image: &[u8], rva: u32, expected: &[u8]) -> Result<(), String> {
    // Both hash-pinned images: .text RVA1000/raw400, verified from PE headers.
    let offset = rva.checked_sub(0xc00).ok_or("text RVA underflow")? as usize;
    if image.get(offset..offset + expected.len()) != Some(expected) {
        return Err(format!("instruction mismatch at {rva:#x}"));
    }
    Ok(())
}
fn branch(image: &[u8], rva: u32, opcode: u8, target: u32) -> Result<(), String> {
    let displacement =
        i32::try_from(i64::from(target) - i64::from(rva) - 5).map_err(|_| "rel32")?;
    let mut expected = [opcode, 0, 0, 0, 0];
    expected[1..].copy_from_slice(&displacement.to_le_bytes());
    bytes(image, rva, &expected)
}

fn same_byte_target(rva: u32, leaf: &[u8]) -> Result<i64, String> {
    let leaf = leaf.get(..14).ok_or("release leaf truncated")?;
    if leaf[..3] != [0x0f, 0xb6, 0x05] || leaf[7..9] != [0x88, 0x0d] || leaf[13] != 0xc3 {
        return Err("release leaf is not byte read/write/return".into());
    }
    let read = i64::from(rva) + 7 + i64::from(i32::from_le_bytes(leaf[3..7].try_into().unwrap()));
    let write =
        i64::from(rva) + 13 + i64::from(i32::from_le_bytes(leaf[9..13].try_into().unwrap()));
    if read != write {
        return Err("release leaf does not read and write the same byte".into());
    }
    Ok(read)
}

fn core_rdata_pointer(image: &[u8], rva: u32, target: u32) -> Result<(), String> {
    // Independently read PE section: .rdata RVA9B7000/raw9B5C00,
    // virtual size1A8124. This mapping never applies to .text or .data.
    let end = rva.checked_add(8).ok_or("rdata range overflow")?;
    if rva < 0x9b7000 || end > 0xb5f124 {
        return Err("outside pinned Core rdata".into());
    }
    let offset = (rva - 0x1400) as usize;
    let expected = (0x180000000u64 + u64::from(target)).to_le_bytes();
    if image.get(offset..offset + 8) != Some(&expected) {
        return Err(format!("vtable pointer mismatch at {rva:#x}"));
    }
    Ok(())
}

const SHUTDOWN_PRE_WAIT: [u8; 117] = [
    0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x80, 0xb9, 0xa4, 0x02, 0x00, 0x00, 0x00, 0x48, 0x8b, 0xd9,
    0x0f, 0x84, 0x63, 0x01, 0x00, 0x00, 0xc6, 0x81, 0xa6, 0x02, 0x00, 0x00, 0x01, 0x48, 0x8d, 0x54,
    0x24, 0x30, 0xc6, 0x81, 0xa4, 0x02, 0x00, 0x00, 0x00, 0x48, 0x81, 0xc1, 0xb0, 0x00, 0x00, 0x00,
    0xc7, 0x44, 0x24, 0x30, 0x01, 0x00, 0x00, 0x00, 0x44, 0x0f, 0xb6, 0x44, 0x24, 0x30, 0x48, 0x89,
    0x7c, 0x24, 0x38, 0xe8, 0x28, 0xc5, 0x00, 0x00, 0x48, 0x8b, 0x03, 0x48, 0x8b, 0xcb, 0xc6, 0x83,
    0xa7, 0x02, 0x00, 0x00, 0x01, 0xff, 0x50, 0x48, 0xe8, 0xa3, 0xbf, 0xf6, 0xff, 0x48, 0x8d, 0x8b,
    0x90, 0x02, 0x00, 0x00, 0xe8, 0xf7, 0x47, 0xfc, 0xff, 0x48, 0x8b, 0x8b, 0x50, 0x02, 0x00, 0x00,
    0xe8, 0x7b, 0x2f, 0xe6, 0xff,
];

fn shutdown_pre_wait(core: &[u8]) -> Result<(), String> {
    bytes(core, 0x63e160, &SHUTDOWN_PRE_WAIT)?;
    for (rva, target) in [
        (0x63e1a3, 0x64a6d0),
        (0x63e1b8, 0x5aa160),
        (0x63e1c4, 0x6029c0),
        (0x63e1d0, 0x4a1150),
    ] {
        branch(core, rva, 0xe8, target)?;
    }
    Ok(())
}

fn context_world_path(core: &[u8]) -> Result<Value, String> {
    for (rva, opcode, target) in [
        (0x752d50, 0xe9, 0x58fc60),
        (0x752d60, 0xe9, 0x58fd10),
        (0x58fcbe, 0xe8, 0x58fb40),
        (0x58fb91, 0xe8, 0x7503e0),
        (0x7503e9, 0xe8, 0x454d0),
        (0x471b0, 0xe8, 0x63da20),
        (0x471bd, 0xe8, 0x63e160),
        (0x750507, 0xe9, 0x750600),
        (0x750609, 0xe8, 0x639160),
        (0x75063f, 0xe8, 0x63ba90),
    ] {
        branch(core, rva, opcode, target)?;
    }
    // Complete small constructor: RBX carries the incoming allocation through
    // its base constructor, table store and RAX return under the Win64 ABI.
    bytes(
        core,
        0x7503e0,
        &[
            0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0xd9, 0xe8, 0xe2, 0x50, 0x8f, 0xff,
            0x48, 0x8d, 0x05, 0x7b, 0x4b, 0x2f, 0x00, 0x48, 0x89, 0x03, 0x48, 0x8b, 0xc3, 0x48,
            0x83, 0xc4, 0x20, 0x5b, 0xc3,
        ],
    )?;
    bytes(
        core,
        0x58fb89,
        &[
            0x48, 0x85, 0xc0, 0x74, 0x15, 0x48, 0x8b, 0xc8, 0xe8, 0x4a, 0x08, 0x1c, 0x00, 0x48,
            0x89, 0x43, 0x08,
        ],
    )?;
    for (slot, target) in [(0, 0x45960), (0x10, 0x470b0), (0x98, 0x7504c0)] {
        core_rdata_pointer(core, 0xa44f70 + slot, target)?;
    }
    // Conditional +10 then conditional deleting virtual0. Pointers are reloaded
    // after calls: the bytes do not prove concurrent pointer/vtable stability.
    bytes(
        core,
        0x58fd6b,
        &[
            0x48, 0x8b, 0x4b, 0x08, 0x48, 0x8d, 0x05, 0x72, 0x04, 0x48, 0x00, 0x48, 0x89, 0x03,
            0x48, 0x85, 0xc9, 0x74, 0x13, 0xe8, 0x0d, 0x65, 0xab, 0xff, 0x84, 0xc0, 0x74, 0x0a,
            0x48, 0x8b, 0x4b, 0x08, 0x48, 0x8b, 0x01, 0xff, 0x50, 0x10, 0x48, 0x8b, 0x4b, 0x08,
            0x48, 0x85, 0xc9, 0x74, 0x0a, 0x48, 0x8b, 0x01, 0xba, 0x01, 0x00, 0x00, 0x00, 0xff,
            0x10,
        ],
    )?;
    bytes(core, 0x46290, &[0x0f, 0xb6, 0x41, 0x40, 0xc3])?;
    bytes(
        core,
        0x470b0,
        &[
            0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x80, 0x79, 0x40, 0x00, 0x48, 0x8b, 0xd9, 0x0f,
            0x84, 0x95, 0x01, 0x00, 0x00,
        ],
    )?;
    // Contiguous block including both branches and the reload before deletion.
    bytes(
        core,
        0x471a7,
        &[
            0x48, 0x8b, 0x4b, 0x60, 0x48, 0x85, 0xc9, 0x74, 0x29, 0xe8, 0x6b, 0x68, 0x5f, 0x00,
            0x84, 0xc0, 0x74, 0x09, 0x48, 0x8b, 0x4b, 0x60, 0xe8, 0x9e, 0x6f, 0x5f, 0x00, 0x48,
            0x8b, 0x4b, 0x60, 0x48, 0x85, 0xc9, 0x74, 0x0a, 0x48, 0x8b, 0x01, 0xba, 0x01, 0x00,
            0x00, 0x00, 0xff, 0x10,
        ],
    )?;
    bytes(
        core,
        0x63da20,
        &[0x0f, 0xb6, 0x81, 0xa4, 0x02, 0x00, 0x00, 0xc3],
    )?;
    // This block stores its RBX receiver's virtual98 result at RBX+60.
    // RBX identity and continuity from a constructor are not established here.
    bytes(
        core,
        0x46d24,
        &[
            0x48, 0x8b, 0x03, 0x48, 0x8b, 0xcb, 0xff, 0x90, 0x98, 0, 0, 0, 0x48, 0x89, 0x43, 0x60,
        ],
    )?;
    bytes(
        core,
        0x7504f7,
        &[
            0x48, 0x85, 0xc0, 0x74, 0x10, 0x48, 0x8b, 0xd3, 0x48, 0x8b, 0xc8, 0x48, 0x83, 0xc4,
            0x30, 0x5b, 0xe9, 0xf4, 0x00, 0x00, 0x00,
        ],
    )?;
    bytes(
        core,
        0x750600,
        &[
            0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0xd9, 0xe8, 0x52, 0x8b, 0xee, 0xff,
            0x48, 0x8d, 0x05, 0x4b, 0x4b, 0x2f, 0x00, 0x48, 0x89, 0x03, 0x48, 0x8b, 0xc3, 0x48,
            0x83, 0xc4, 0x20, 0x5b, 0xc3,
        ],
    )?;
    core_rdata_pointer(core, 0xa45160, 0x750630)?;
    bytes(core, 0x63e160, &[0x40, 0x53, 0x48, 0x83, 0xec, 0x20])?;
    // Separate prologue shape after return/padding; incoming CFG edges are not
    // enumerated, so this does not establish all function boundaries.
    bytes(
        core,
        0x63e2d9,
        &[
            0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3, 0xcc, 0x48, 0x83, 0xec, 0x28,
        ],
    )?;
    Ok(json!({
        "destroy_thunk_rva":0x752d60,"destroy_body_rva":0x58fd10,
        "export_name_basis":"DllDestroyGameContext name/RVA obtained independently with LLVM; export table not parsed by this audit",
        "local_rbx_field_offsets":{"58FB96":8,"58FD6B":8,"471A7":0x60,"46D30":0x60},
        "table_store_function_7503e0_rva":0x7503e0,"table_stored_at_7503f5_rva":0xa44f70,
        "table_a44f70_slots":{"virtual0":0x45960,"virtual10":0x470b0,"virtual98":0x7504c0},
        "subobject_flag_reader":{"rva":0x46290,"byte_offset":0x40},
        "table_store_function_750600_rva":0x750600,
        "call_from_750609_rva":0x639160,"table_stored_at_750615_rva":0xa45160,
        "table_a45160_virtual0":0x750630,"call_from_virtual0":0x63ba90,
        "conditional_target_shutdown_call_rva":0x471bd,"target_shutdown_rva":0x63e160,
        "target_flag_reader":{"rva":0x63da20,"byte_offset":0x2a4},
        "subsequent_virtual0_call_rva":0x471d3,"virtual0_edx":1,
        "static_order_scope":"in the verified471A7 caller block, normal63E160 return reaches RBX+60 reload and conditional virtual0 with EDX1; runtime occurrence and pointer equality across calls not certified",
        "source_type_name":"not established; constructor/vtable identity only",
        "separate_entry_shape":{"rva":0x63e2e0,"call_rva":0x63e2e4,"target":0x4afc0,"connection_to_shutdown":"not established; separate prologue shape after return and padding, incoming edges not enumerated"},
        "unestablished":["host destroy-function slot resolved to export at runtime","executed vtable and receiver stability","470B0 entry to471A7 block reachability and RBX continuity","46D24 RBX identity with constructor/tableA44F70 subobject or471A7 RBX","58FD10 to58FD6B block reachability","58FB40 to58FB89 block reachability and RBX continuity","7504C0 to7504F7 block reachability","all submissions sealed and pending jobs published","synchronous callbacks exited","Close/Rebuild tied to native destruction"]
    }))
}

pub fn audit(core: &[u8], host: &[u8]) -> Result<Value, String> {
    let core_sha = format!("{:x}", Sha256::digest(core));
    let host_sha = format!("{:x}", Sha256::digest(host));
    if core_sha != civ6_native_bridge_rs::PINNED_GAMECORE_SHA256 || host_sha != HOST_SHA {
        return Err("not both pinned primary files; no ABI interpretation".into());
    }
    shutdown_pre_wait(core)?;
    let core_calls = [
        (
            0x58ff73,
            0xe9,
            0x7bbf0,
            "release virtual target forwards only a byte switch",
        ),
        (0x49c807, 0xe8, 0x992160, "create list"),
        (0x4a7a4b, 0xe8, 0x4a1100, "Governor Analyzer submit"),
        (0x4a7a53, 0xe8, 0x4a10b0, "Governor Analyzer spawn"),
        (0x4a1145, 0xe8, 0x992210, "submit wrapper"),
        (0x4a10f9, 0xe9, 0x992280, "spawn wrapper"),
        (0x4a1713, 0xe8, 0x9922c0, "ordinary wait"),
        (
            0x63e1d0,
            0xe8,
            0x4a1150,
            "world shutdown to manager cleanup",
        ),
        (0x4a1191, 0xe8, 0x9922c0, "shutdown wait"),
        (0x4a1209, 0xe8, 0x9921d0, "release list"),
        (
            0x63e2e4,
            0xe8,
            0x4afc0,
            "separate entry calls 4AFC0; no shutdown-tail evidence",
        ),
        (
            0x49bce1,
            0xe8,
            0x992190,
            "manager destructor releases job set",
        ),
    ];
    let mut witnesses = Vec::new();
    for (rva, op, target, purpose) in core_calls {
        branch(core, rva, op, target)?;
        witnesses.push(json!({"image":"GameCore", "rva":rva, "file_offset":rva-0xc00,"target":target,"purpose":purpose}));
    }
    for (rva, op, target) in [
        (0x986030, 0xe9, 0x985550),
        (0x986040, 0xe9, 0x985660),
        (0x986050, 0xe9, 0x9857b0),
        (0x986072, 0xe8, 0x985b20),
        (0x986080, 0xe9, 0x985bf0),
        (0x985e4b, 0xe8, 0xe3ea70),
    ] {
        branch(host, rva, op, target)?;
        witnesses.push(
            json!({"image":"CivilizationVI", "rva":rva,"file_offset":rva-0xc00,"target":target}),
        );
    }
    bytes(core, 0x4a1161, &[0xc7, 0x81, 0xa8, 0x38, 0, 0, 3, 0, 0, 0])?;
    bytes(core, 0x4a117f, &[0x48, 0x81, 0xc1, 0, 4, 0, 0])?;
    bytes(core, 0x58ff70, &[0x0f, 0xb6, 0xca])?;
    bytes(
        core,
        0x7bbf0,
        &[
            0x0f, 0xb6, 0x05, 0x09, 0x62, 0xae, 0, 0x88, 0x0d, 0x03, 0x62, 0xae, 0, 0xc3,
        ],
    )?;
    bytes(core, 0x63e166, &[0x80, 0xb9, 0xa4, 0x02, 0, 0, 0])?;
    // Save the incoming receiver in RBX, then skip to63E2D9 when its byte is zero.
    bytes(
        core,
        0x63e16d,
        &[0x48, 0x8b, 0xd9, 0x0f, 0x84, 0x63, 0x01, 0, 0],
    )?;
    bytes(
        core,
        0x63e1a8,
        &[
            0x48, 0x8b, 0x03, 0x48, 0x8b, 0xcb, 0xc6, 0x83, 0xa7, 0x02, 0, 0, 1,
        ],
    )?;
    bytes(core, 0x63e1b5, &[0xff, 0x50, 0x48])?;
    bytes(core, 0x63e1c9, &[0x48, 0x8b, 0x8b, 0x50, 0x02, 0, 0])?;
    // The two verified RIP-relative instructions access the same byte.
    let leaf = core
        .get((0x7bbf0 - 0xc00)..(0x7bbf0 - 0xc00 + 14))
        .ok_or("release leaf range")?;
    let read_target = same_byte_target(0x7bbf0, leaf)?;
    bytes(host, 0x985e50, &[0x48, 0xc7, 0x47, 8, 0, 0, 0, 0])?;
    bytes(host, 0x985e12, &[0x48, 0x8b, 7, 0x48, 0x89, 0x47, 8])?;
    Ok(json!({
        "schema":"todo880-native-job-lifecycle-static-v4",
        "core_sha256":core_sha,"host_sha256":host_sha,"witnesses":witnesses,
        "manager":{"from_world_offset":0x250,"job_set_offset":0x400,"list_id_offset":0x410,"lifecycle_state_offset":0x38a8},
        "abi":{
            "push":{"rcx":"list ID (native host uses low32, IDs1..64)","rdx":"native job object", "r8":"manager+400 job-set address", "r9":"pointer to 16-byte metadata copy"},
            "wait":{"rcx":"same job-set address", "rdx":"pointer to 16-byte metadata copy", "result":"normal return, not RAX=true; host clears set+8 after native arena wait"}
        },
        "receipt_candidates":{"wait_return_core":0x4a1196,"conditional_host_callback_merge_core":0x4a11b3,"free_list_return_core":0x4a120e,"host_wait_pointer_clear":0x985e50},
        "file_offset_basis":"SHA-pinned images with independently verified text RVA1000/raw400; Core rdata RVA9B7000/raw9B5C00 virtual size1A8124 and preferred ImageBase180000000; not a general PE parser",
        "release_leaf":{"wrapper_rva":0x58ff70,"leaf_rva":0x7bbf0,"instruction_bytes":14,"same_byte_target_rva":read_target,"behavior":"non-atomic read then write of one RIP-relative byte; not world teardown","world_shutdown_link_evidence":"not established by this audit"},
        "context_world_path":context_world_path(core)?,
        "shutdown_guard":{"entry_rva":0x63e160,"compare_rva":0x63e166,"compare_base":"RCX at63E166","receiver_byte_offset":0x2a4,"compare_value":0,"zero_branch_target":0x63e2d9,"virtual_call_rva":0x63e1b5,"virtual_slot":0x48,"virtual_load_base":"RBX saved from entry RCX at63E16D; contiguous prefix has no later RBX assignment, conditional on normal-return callees obeying Win64 nonvolatile ABI","local_byte_store_offset":0x2a7,"receiver_type_evidence":"constructor/table path recorded separately; source-language type name not established","manager_load_base":"same conditional RBX+250 at63E1C9 under the stated ABI assumption","caller_evidence":"verified conditional call at471BD in RBX+60 block; receiver identity across blocks unestablished","runtime_lifetime_order_evidence":"not established by this audit"},
        "shutdown_pre_wait_flow":{
            "entry_rva":0x63e160,"contiguous_bytes":SHUTDOWN_PRE_WAIT.len(),
            "zero_guard_exits_prefix_to":0x63e2d9,
            "normal_return_call_sites":[0x63e1a3,0x63e1b5,0x63e1b8,0x63e1c4,0x63e1d0],
            "receiver_flag_stores":[{"rva":0x63e176,"offset":0x2a6,"value":1},{"rva":0x63e182,"offset":0x2a4,"value":0},{"rva":0x63e1ae,"offset":0x2a7,"value":1}],
            "scope":"from entry63E160 only, nonzero guard falls through these instructions in this block, conditional on normal returns and Win64 RBX preservation; not a runtime execution receipt",
            "entry_guard_clear":{"rva":0x63e182,"offset":0x2a4,"value":0,"conditional_effect":"another entry with this byte still zero exits via63E2D9; other sites re-setting the byte not investigated"},
            "unestablished":["incoming control-flow edges into the middle of the block","callee RBX preservation in actual native execution","callback return and receiver lifetime","flag stores preventing all native submissions","synchronous callback/outer caller drainage","same job-set pending publication and full join"]
        },
        "limits":[
            "state3 is not an admission seal: Push wrapper does not check it",
            "set+8 zero is not permanent: a later Push reactivates it from set+0",
            "require list publication/empty-pending proof as well as same-set Wait",
            "FreeJobList recycles list nodes; it is not a join or cancellation proof",
            "native arena wait has current-arena early-return branches; classify the executed path",
            "worker-set join does not by itself join synchronous 4A7AD0 callback or other call sites",
            "no new submission for this world may pass between receipt and destruction"
            ,"contiguous pre-wait prefix provides conditional static call/register flow; runtime return/order and object lifetimes are not certified"
            ,"release virtual target 58FF70/7BBF0 is only a byte switch, not world shutdown"
            ,"entry 63E160 skips to63E2D9 when RCX+2A4 is zero; its static471BD caller is recorded, runtime receiver stability and Close/Rebuild order remain unestablished"
        ],
        "certificate_issued":false,"runtime_calls":0,"game_runs":0,"ai_acceptance":false,
        "zero_count_basis":"audit reads two files only; runtime/game zero are structural declarations, not measured native counters"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_offsets_are_interpreted_for_unpinned_inputs() {
        assert!(audit(&[], &[]).unwrap_err().contains("pinned"));
        assert!(audit(b"MZ", b"PE").is_err());
    }

    #[test]
    fn instruction_helpers_reject_mismatch_and_range_and_accept_negative_rel32() {
        let mut image = vec![0; 0x200];
        image[0x100..0x105].copy_from_slice(&[0xe9, 0xeb, 0xff, 0xff, 0xff]);
        assert!(branch(&image, 0xd00, 0xe9, 0xcf0).is_ok());
        assert!(branch(&image, 0xd00, 0xe8, 0xcf0).is_err());
        assert!(branch(&image, 0xd00, 0xe9, 0xcf1).is_err());
        assert!(bytes(&image, 0xdff, &[0, 0]).is_err());
        assert!(bytes(&image, 0xbff, &[0]).is_err());
        let leaf = [
            0x0f, 0xb6, 0x05, 0x09, 0x62, 0xae, 0, 0x88, 0x0d, 0x03, 0x62, 0xae, 0, 0xc3,
        ];
        assert_eq!(same_byte_target(0x7bbf0, &leaf).unwrap(), 0xb61e00);
        let mut changed = leaf;
        changed[9] += 1;
        assert!(same_byte_target(0x7bbf0, &changed).is_err());
        assert!(same_byte_target(0x7bbf0, &leaf[..13]).is_err());
    }

    #[test]
    fn rdata_pointer_uses_its_own_section_and_rejects_truncation_and_crossing() {
        // Last complete pointer of the pinned virtual range. A text mapping
        // would read 0x800 later, so it cannot accidentally pass this fixture.
        let rva = 0xb5f11c;
        let offset = (rva - 0x1400) as usize;
        let mut image = vec![0; offset + 8];
        image[offset..].copy_from_slice(&0x180750630u64.to_le_bytes());
        assert!(core_rdata_pointer(&image, rva, 0x750630).is_ok());
        assert!(core_rdata_pointer(&image, rva, 0x750631).is_err());
        assert!(core_rdata_pointer(&image[..image.len() - 1], rva, 0x750630).is_err());
        assert!(core_rdata_pointer(&image, rva + 1, 0x750630).is_err());
        assert!(core_rdata_pointer(&image, 0x9b6fff, 0x750630).is_err());
        assert!(core_rdata_pointer(&image, u32::MAX, 0x750630).is_err());
    }

    #[test]
    fn contiguous_shutdown_flow_rejects_changes_between_old_anchors() {
        let offset = 0x63e160 - 0xc00;
        let mut image = vec![0; offset + SHUTDOWN_PRE_WAIT.len()];
        image[offset..].copy_from_slice(&SHUTDOWN_PRE_WAIT);
        assert!(shutdown_pre_wait(&image).is_ok());
        // Former isolated anchors did not cover this ADD RCX,B0 immediate.
        // Altering intervening instructions must invalidate the flow claim.
        image[offset + (0x63e18c - 0x63e160)] ^= 1;
        assert!(shutdown_pre_wait(&image).is_err());
        image[offset + (0x63e18c - 0x63e160)] ^= 1;
        assert!(shutdown_pre_wait(&image[..image.len() - 1]).is_err());
    }
}
