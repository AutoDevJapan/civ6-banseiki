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

pub fn audit(core: &[u8], host: &[u8]) -> Result<Value, String> {
    let core_sha = format!("{:x}", Sha256::digest(core));
    let host_sha = format!("{:x}", Sha256::digest(host));
    if core_sha != civ6_native_bridge_rs::PINNED_GAMECORE_SHA256 || host_sha != HOST_SHA {
        return Err("not both pinned primary files; no ABI interpretation".into());
    }
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
            "call to 4AFC0; role not established by this audit",
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
        "schema":"todo880-native-job-lifecycle-static-v2",
        "core_sha256":core_sha,"host_sha256":host_sha,"witnesses":witnesses,
        "manager":{"from_world_offset":0x250,"job_set_offset":0x400,"list_id_offset":0x410,"lifecycle_state_offset":0x38a8},
        "abi":{
            "push":{"rcx":"list ID (native host uses low32, IDs1..64)","rdx":"native job object", "r8":"manager+400 job-set address", "r9":"pointer to 16-byte metadata copy"},
            "wait":{"rcx":"same job-set address", "rdx":"pointer to 16-byte metadata copy", "result":"normal return, not RAX=true; host clears set+8 after native arena wait"}
        },
        "receipt_candidates":{"wait_return_core":0x4a1196,"conditional_host_callback_merge_core":0x4a11b3,"free_list_return_core":0x4a120e,"host_wait_pointer_clear":0x985e50},
        "file_offset_basis":"SHA-pinned images with independently verified text RVA1000/raw400; not a general PE parser",
        "release_leaf":{"wrapper_rva":0x58ff70,"leaf_rva":0x7bbf0,"instruction_bytes":14,"same_byte_target_rva":read_target,"behavior":"non-atomic read then write of one RIP-relative byte; not world teardown","world_shutdown_link_evidence":"not established by this audit"},
        "shutdown_guard":{"entry_rva":0x63e160,"compare_rva":0x63e166,"compare_base":"RCX at63E166","receiver_byte_offset":0x2a4,"compare_value":0,"zero_branch_target":0x63e2d9,"virtual_call_rva":0x63e1b5,"virtual_slot":0x48,"virtual_load_base":"RBX at63E1A8; continuity from63E16D not established","local_byte_store_offset":0x2a7,"receiver_type_evidence":"not established by this audit","manager_load_base":"RBX+250 at63E1C9; continuity from earlier anchors not established","later_call_rva":0x63e2e4,"later_call_target":0x4afc0,"later_call_order_evidence":"higher RVA only; executed order not established","caller_and_lifetime_order_evidence":"not established by this audit"},
        "limits":[
            "state3 is not an admission seal: Push wrapper does not check it",
            "set+8 zero is not permanent: a later Push reactivates it from set+0",
            "require list publication/empty-pending proof as well as same-set Wait",
            "FreeJobList recycles list nodes; it is not a join or cancellation proof",
            "native arena wait has current-arena early-return branches; classify the executed path",
            "worker-set join does not by itself join synchronous 4A7AD0 callback or other call sites",
            "no new submission for this world may pass between receipt and destruction"
            ,"indirect call 63E1B5 has lower RVA than manager-call63E1D0; register continuity, runtime return/order and object lifetimes not certified"
            ,"release virtual target 58FF70/7BBF0 is only a byte switch, not world shutdown"
            ,"entry 63E160 skips to63E2D9 when RCX+2A4 is zero; receiver type, caller and Close/Rebuild order remain unestablished"
        ],
        "certificate_issued":false,"runtime_calls":0,"game_runs":0,"ai_acceptance":false
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
}
