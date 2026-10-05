//! A reproducible, disk-only candidate for the missing resident bootstrap.
//! Does not install a hook or expose an address as a callable game ABI.
use super::*;
use sha2::{Digest, Sha256};

const EXE_SHA: &str = "e7450823cc8e00468cff7b9d7b97c63140eae38ae1d774ba4efa437556c42d63";
const CORE_SHA: &str = "324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c";

fn identity(bytes: &[u8], expected: &str) -> Result<(), String> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        return Err(format!(
            "pinned PE SHA256 differs: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn bytes_at<'a>(pe: &Pe<'a>, rva: u32, count: usize) -> Result<&'a [u8], String> {
    let offset = pe
        .offset(rva)
        .ok_or_else(|| format!("unmapped RVA 0x{rva:x}"))?;
    // Reject a range that crosses a section's file-backed extent.
    let last = rva
        .checked_add(
            u32::try_from(count.checked_sub(1).ok_or("empty range")?)
                .map_err(|_| "range too large")?,
        )
        .ok_or("RVA overflow")?;
    if pe.offset(last) != offset.checked_add(count - 1) {
        return Err(format!(
            "RVA range at 0x{rva:x} is not contiguous file bytes"
        ));
    }
    at(pe.bytes, offset, count)
}

fn exact(pe: &Pe<'_>, rva: u32, expected: &[u8]) -> Result<(), String> {
    if bytes_at(pe, rva, expected.len())? != expected {
        return Err(format!("instruction bytes differ at RVA 0x{rva:x}"));
    }
    Ok(())
}

fn rel32(code: &[u8], rva: u32, opcode: u8) -> Result<u32, String> {
    if code.len() != 5 || code[0] != opcode {
        return Err(format!("expected {opcode:02x} rel32 at RVA 0x{rva:x}"));
    }
    let delta = i32::from_le_bytes(code[1..5].try_into().unwrap());
    u32::try_from(i64::from(rva) + 5 + i64::from(delta)).map_err(|_| "branch RVA overflow".into())
}

fn edge(pe: &Pe<'_>, site: u32, opcode: u8, target: u32) -> Result<(), String> {
    let actual = rel32(bytes_at(pe, site, 5)?, site, opcode)?;
    if actual != target {
        return Err(format!(
            "branch at 0x{site:x} targets 0x{actual:x}, expected 0x{target:x}"
        ));
    }
    Ok(())
}

fn verify(exe: &Pe<'_>, core: &Pe<'_>) -> Result<(), String> {
    // LoadLibraryW RCX=wide filename, RAX=HMODULE. The post-return site is
    // before GetProcAddress and before the lazy factory call in this thread.
    edge(exe, 0xf8572, 0xe8, 0x989970)?;
    exact(exe, 0x989970, &[0x48, 0xff, 0x25, 0xd1, 0xcf, 0x60, 0])?;
    exact(exe, 0xf8577, &[0x48, 0x89, 0x05, 0x5a, 0xdb, 0x1d, 0x02])?;
    edge(exe, 0xf858d, 0xe8, 0x9898f0)?;
    exact(exe, 0xfba318, b"DllCreateGameContext\0")?;
    exact(exe, 0xfba330, b"DllDestroyGameContext\0")?;
    exact(exe, 0xf8451, &[0x48, 0x8b, 0x0d, 0x90, 0xe0, 0x1d, 0x02])?;
    exact(exe, 0xf845d, &[0xff, 0xd1])?;
    // Same caller: teardown -> DLL loader -> lazy factory -> virtual setup.
    edge(exe, 0x24b523, 0xe8, 0xf8e30)?;
    edge(exe, 0x24b52d, 0xe8, 0xf8510)?;
    edge(exe, 0x24b545, 0xe8, 0xf8430)?;
    exact(exe, 0x24b5bf, &[0x48, 0x8b, 0x01, 0xff, 0x50, 0x60])?;
    edge(core, 0x752d50, 0xe9, 0x58fc60)?;
    edge(core, 0x58fcbe, 0xe8, 0x58fb40)?;
    edge(core, 0x58fb91, 0xe8, 0x7503e0)?;
    edge(core, 0x752d60, 0xe9, 0x58fd10)?;

    // Four real call sites share release before context destruction. This is
    // NOT a proof that the four user actions all reach these sites.
    for (release, destroy) in [
        (0x245cea, 0x245cef),
        (0x24b51e, 0x24b523),
        (0x24cab6, 0x24cabb),
        (0x24d1d2, 0x24d1d7),
    ] {
        edge(exe, release, 0xe8, 0xf7110)?;
        edge(exe, destroy, 0xe8, 0xf8e30)?;
    }
    exact(exe, 0xf7110, &[0x40, 0x53, 0x48, 0x83, 0xec, 0x20])?;
    exact(exe, 0xf7128, &[0x48, 0x8b, 0x8b, 0x50, 0x04, 0, 0])?;
    exact(exe, 0xf7137, &[0x33, 0xd2, 0xff, 0x90, 0xa0, 1, 0, 0])?;
    exact(exe, 0xf715e, &[0x48, 0xff, 0x60, 0x08])?;
    // A second path has the SAME release body. Do not select f7110 as a
    // universal close hook merely because it has four direct callers.
    exact(exe, 0xf7865, &[0x33, 0xd2, 0xff, 0x90, 0xa0, 1, 0, 0])?;
    exact(exe, 0xf7887, &[0xff, 0x50, 0x08])?;
    edge(exe, 0xf8e43, 0xe8, 0x29c2a0)?;
    exact(exe, 0xf8e60, &[0xff, 0xd0])?;
    edge(exe, 0x971b11, 0xe8, 0xdbd83c)?;
    edge(exe, 0x971cb7, 0xe8, 0xdbd83c)?;
    exact(exe, 0xdbd83c, &[0xff, 0x25, 0xae, 0x8a, 0x1d, 0])?;
    Ok(())
}

pub(super) fn audit(exe_path: &Path, core_path: &Path) -> Result<(), String> {
    let exe_bytes = fs::read(exe_path).map_err(|e| format!("{}: {e}", exe_path.display()))?;
    let core_bytes = fs::read(core_path).map_err(|e| format!("{}: {e}", core_path.display()))?;
    identity(&exe_bytes, EXE_SHA)?;
    identity(&core_bytes, CORE_SHA)?;
    let exe = Pe::parse(&exe_bytes)?;
    let core = Pe::parse(&core_bytes)?;
    verify(&exe, &core)?;
    println!("static_primary_checks=passed\nexe_sha256={EXE_SHA}\ngamecore_sha256={CORE_SHA}");
    println!("selected_startup_candidate=exe_post_LoadLibraryW_RVA_0xf8577");
    println!("startup_argument=RAX_loaded_HMODULE; factory_export_RVA_0x752d50_ignores_incoming_argument_registers");
    println!(
        "preclose_candidates=exe_RVA_0xf7110_and_0xf7830; RCX_receiver_with_interface_at_0x450"
    );
    println!("context_destroy_is_later=exe_RVA_0xf8e60; RCX_context; core_RVA_0x752d60");
    println!("lua_close_sites=exe_RVA_0x971b11_and_0x971cb7; RCX_lua_State_from_receiver_0x10");
    println!("missing=resident_activation_before_loader; all_user_paths_to_preclose; gameplay_Lua_identity_and_thread_order; runtime_rearm_before_new_RESET");
    println!("execution_ready=false\nauto_enabled=false\nlive_start_count=0\nreal_hook_reload_normal_userpath_receipts=0");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_calls_distinguish_backward_targets_opcode_and_overflow() {
        assert_eq!(
            rel32(&[0xe8, 0xeb, 0xff, 0xff, 0xff], 0x1010, 0xe8).unwrap(),
            0x1000
        );
        assert!(rel32(&[0xe9, 0xeb, 0xff, 0xff, 0xff], 0x1010, 0xe8).is_err());
        assert!(rel32(&[0xe8, 0, 0, 0, 0], u32::MAX, 0xe8).is_err());
        assert!(rel32(&[0xe8, 0, 0, 0, 0x80], 0, 0xe8).is_err());
        assert!(rel32(&[0xe8, 0, 0], 0, 0xe8).is_err());
    }

    #[test]
    fn changed_or_empty_images_cannot_pass_pinned_identity() {
        // FIPS 180-4 SHA-256 vector, not a fabricated WinAPI success.
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        identity(b"abc", expected).unwrap();
        assert!(identity(b"abd", expected).is_err());
        assert!(identity(b"", EXE_SHA).is_err());
        assert!(identity(b"", CORE_SHA).is_err());
    }
}
