//! Fixed-image witnesses, not a generic PE parser or an ABI authorization.
//! .text RVA=1000, raw=400 were independently checked with civ6-pe/LLVM.
//! Full SHA verification precedes use of that pinned mapping. No bootstrap code.
use civ6_native_bridge_rs::PINNED_GAMECORE_SHA256;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const BRANCHES: &[(u32, u8, u32, &str)] = &[
    (
        0x40263c,
        0xe8,
        0x4a7f70,
        "registered AI Governors receiver lookup",
    ),
    (
        0x402672,
        0xe8,
        0x404770,
        "register AI callback in owner callback collection",
    ),
    (0x4a7b1b, 0xe8, 0x2de920, "AI owner to Governors"),
    (0x4a7d54, 0xe8, 0x2de960, "AI owned Governor lookup"),
    (0x4a7d67, 0xe8, 0x2ddbb0, "AI CanPromote"),
    (0x4a7ed3, 0xe8, 0x30cbe0, "AI operation construction"),
    (
        0x4a7ef0,
        0xe8,
        0x30c100,
        "AI owner validation (not execution)",
    ),
    (0x4a7f0f, 0xe8, 0x038990, "AI operation wrapper copy"),
    (0x4a7f18, 0xe8, 0x038c80, "AI direct execute"),
    (0x038c87, 0xe9, 0x30d330, "wrapper to common dispatcher"),
    (0x30d366, 0xe8, 0x044d90, "dispatcher player lookup"),
    (0x30d3dd, 0xe8, 0x30c100, "dispatcher revalidates owner"),
    (0x30ebf8, 0xe8, 0x2de960, "dispatcher owned Governor lookup"),
    (0x30ec0f, 0xe8, 0x2ddbb0, "dispatcher CanPromote"),
    (
        0x30ec24,
        0xe8,
        0x2df340,
        "dispatcher mutation; never call directly",
    ),
    (0x2ddbcb, 0xe8, 0x2dea90, "CanPromote reads aggregate Spent"),
    (
        0x2ddc09,
        0xe8,
        0x259980,
        "CanPromote rejects acquired promotion",
    ),
    (
        0x2decc0,
        0xe8,
        0x2598f0,
        "Spent counts actual Governor flags",
    ),
];

fn text_bytes(bytes: &[u8], rva: u32, len: usize) -> Result<&[u8], String> {
    let start = rva.checked_sub(0xc00).ok_or("RVA below pinned text")? as usize;
    bytes
        .get(start..start.checked_add(len).ok_or("offset overflow")?)
        .ok_or_else(|| format!("truncated witness at {rva:#x}"))
}

fn branch(bytes: &[u8], rva: u32, opcode: u8, target: u32) -> Result<(), String> {
    let code = text_bytes(bytes, rva, 5)?;
    let actual = i64::from(rva) + 5 + i64::from(i32::from_le_bytes(code[1..5].try_into().unwrap()));
    if code[0] != opcode || actual != i64::from(target) {
        return Err(format!("branch mismatch at {rva:#x}"));
    }
    Ok(())
}

pub fn audit(bytes: &[u8]) -> Result<Value, String> {
    let digest = format!("{:x}", Sha256::digest(bytes));
    if digest != PINNED_GAMECORE_SHA256 {
        return Err("not the pinned GameCore SHA256; no RVA interpretation".into());
    }
    let mut rows = Vec::new();
    for &(rva, opcode, target, meaning) in BRANCHES {
        branch(bytes, rva, opcode, target)?;
        rows.push(
            json!({"rva":format!("0x{rva:x}"), "file_offset":format!("0x{:x}", rva-0xc00),
            "target_rva":format!("0x{target:x}"), "meaning":meaning}),
        );
    }
    // Different RDX meanings on the adjacent calls are intentional.
    for (rva, expected) in [
        (0x401534, &b"\xff\x50\x10"[..]),
        (0x402649, &b"\x48\x8d\x05\x80\x54\x0a\x00"[..]),
        (
            0x404170,
            &b"\x45\x0f\xb6\x00\x48\x8b\xc1\x8b\x12\x48\x8b\x49\x08\x48\xff\x60\x10"[..],
        ),
        (0x30ec06, &b"\x44\x8b\xc7\x48\x8b\xd0\x48\x8b\xce"[..]),
        (0x30ec1c, &b"\x44\x8b\xc7\x8b\xd3\x48\x8b\xce"[..]),
        (0x4a7e99, &b"\xba\x25\x16\x5e\xaf"[..]),
        (0x4a7eae, &b"\xba\xea\xd4\x4b\x6d"[..]),
        (0x4a7eb8, &b"\xba\x9d\xa9\x3d\x7d"[..]),
        (0x4a7ecd, &b"\x8b\x88\xd8\x00\x00\x00"[..]),
    ] {
        if text_bytes(bytes, rva, expected.len())? != expected {
            return Err(format!("argument witness mismatch at {rva:#x}"));
        }
    }
    Ok(
        json!({"schema":"todo880-ai-operation-static-v1", "sha256":digest,
        "static_witnesses":rows, "argument_witnesses":9,
        "operation_hash":"0x7d3da99d", "governor_parameter_hash":"0xaf5e1625",
        "promotion_parameter_hash":"0x6d4bd4ea",
        "runtime_adapter":"absent", "execution_enabled":false, "payment_authority":false,
        "ai_acceptance":false, "calibration":calibration_plan(),
        "unproved":["natural AI callback thread and synchronized phase", "safe callback installation and lifetime",
            "complete owner/GameInfo/ledger binding", "load boundary and stale operation drainage", "multiplayer ordering"]}),
    )
}

/// Observation plan only. It has no installer or invocation capability.
pub fn calibration_plan() -> Value {
    json!({
        "schema":"todo880-ai-promotion-capture-v1", "mode8_raw_abi_reused":true,
        "runtime_installed":false, "receipt_complete":false, "game_runs":0,
        "limit":{"persons":2,"extra_operations":3,"partial_reload":1},
        "common_caller_rva":"0x401534", "callback_return_rva":"0x401537",
        "context_entry_rva":"0x4a7ad0", "callback_adapter_rva":"0x404170",
        "callback_registration_rva":"0x402649", "required_phase":2,
        "required_samples":[
            {"site":"AI checker entry", "call_rva":"0x4a7d67","return_rva":"0x4a7d6c","callee_rva":"0x2ddbb0","operand_atomic4_aligned":true},
            {"site":"original Spent entry/return", "call_rva":"0x2ddbcb","return_rva":"0x2ddbd0","callee_rva":"0x2dea90","operand_atomic4_aligned":true},
            {"site":"operation constructor", "call_rva":"0x4a7ed3","return_rva":"0x4a7ed8","callee_rva":"0x30cbe0","operand_atomic4_aligned":true},
            {"site":"owner filter entry/return", "call_rva":"0x4a7ef0","return_rva":"0x4a7ef5","callee_rva":"0x30c100","operand_atomic4_aligned":false},
            {"site":"AI execute prefix", "call_rva":"0x4a7f18","callee_rva":"0x38c80","operand_atomic4_aligned":false},
            {"site":"official checker entry/return", "call_rva":"0x30ec0f","return_rva":"0x30ec14","callee_rva":"0x2ddbb0","operand_atomic4_aligned":true},
            {"site":"approval entry/return", "call_rva":"0x30ec24","return_rva":"0x30ec29","callee_rva":"0x2df340","operand_atomic4_aligned":false}
        ],
        "mode8_limit":"entry only; returns and unaligned sites need independently validated observer routing; never widen current atomic patch",
        "post_spent":"fresh naturally observed original 2DEA90 return with same owner; absence means no receipt",
        "registers":["RCX","RDX","R8","R9","original return address","original RAX at return"],
        "required_binding":["OS pid/creation/thread","load generation","natural AI phase","synchronized ledger and full GameInfo","same Governor Base/TITLE/Index","original Spent compared to full native list twice"],
        "zero_permits":true, "payment_authority":false, "ai_acceptance":false
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonpinned_and_truncated_images_never_get_rva_interpretation() {
        assert!(audit(&[]).unwrap_err().contains("SHA256"));
        assert!(audit(b"MZ synthetic is not primary evidence").is_err());
    }
    #[test]
    fn negative_relative_branch_and_tail_are_checked_not_scanned() {
        let mut bytes = vec![0; 0x410];
        bytes[0x400] = 0xe8;
        bytes[0x401..0x405].copy_from_slice(&(-0x105i32).to_le_bytes());
        assert_eq!(branch(&bytes, 0x1000, 0xe8, 0xf00), Ok(()));
        assert!(branch(&bytes, 0x1000, 0xe9, 0xf00).is_err());
        assert!(branch(&bytes, 0x1000, 0xe8, 0xf01).is_err());
        assert!(branch(&bytes[..0x404], 0x1000, 0xe8, 0xf00).is_err());
    }
}
