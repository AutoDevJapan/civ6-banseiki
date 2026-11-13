//! Fixed scheduler handoffs only; no all-producer seal or runtime certificate.
use super::*;
#[path = "native_scheduler_core_fixed.rs"]
mod core;
fn target(
    bytes: &[u8],
    rva: u32,
    prefix: &[u8],
    length: usize,
    offset: usize,
) -> Result<u32, &'static str> {
    if bytes.len() != length || !bytes.starts_with(prefix) || offset + 4 != length {
        return Err("scheduler branch encoding");
    }
    u32::try_from(
        i64::from(rva)
            + length as i64
            + i64::from(i32::from_le_bytes(bytes[offset..].try_into().unwrap())),
    )
    .map_err(|_| "scheduler branch range")
}
pub(super) fn verify_flow(
    mut read: impl FnMut(bool, u32, usize) -> Option<Vec<u8>>,
) -> Result<(), &'static str> {
    for (core, rva, prefix, length, offset, expected) in [
        (true, 0x4a1145, &[0xe8][..], 5, 1, 0x992210),
        (true, 0x4a10f9, &[0xe9][..], 5, 1, 0x992280),
        (true, 0x4a1191, &[0xe8][..], 5, 1, 0x9922c0),
        (true, 0x4a1209, &[0xe8][..], 5, 1, 0x9921d0),
        (false, 0x986442, &[0xe8][..], 5, 1, 0x9858b0),
        (false, 0x986492, &[0xe8][..], 5, 1, 0x985f40),
        (false, 0x9858ff, &[0xe8][..], 5, 1, 0xe3ccf0),
        (false, 0x985f99, &[0xe8][..], 5, 1, 0xe3ccf0),
        (false, 0x985962, &[0xe9][..], 5, 1, 0xe3e8c0),
        (false, 0x9860fb, &[0xe8][..], 5, 1, 0xe3e8c0),
        (false, 0x98613e, &[0xe8][..], 5, 1, 0xe3e8c0),
        (false, 0xe3e98c, &[0xe8][..], 5, 1, 0xe413a0),
        (false, 0xe3ea6b, &[0xe8][..], 5, 1, 0xe3e430),
    ] {
        let b = read(core, rva, length).ok_or("scheduler branch unreadable")?;
        if target(&b, rva, prefix, length, offset)? != expected {
            return Err("scheduler branch target differs");
        }
    }
    for (rva, table) in [(0x1120678, 0xe42470u64), (0x11206b0, 0xe427f0)] {
        let b = read(false, rva + 0x30, 8).ok_or("scheduler table unreadable")?;
        if b.as_slice() != (0x140000000u64 + table).to_le_bytes() {
            return Err("scheduler typed table differs");
        }
    }
    for (rva, expected) in [
        (
            0xe41ad8,
            &[
                0x49, 0x8b, 0x06, 0x49, 0x8d, 0x55, 0xe8, 0x49, 0x8b, 0xce, 0xff, 0x50, 0x30,
            ][..],
        ),
        (
            0xe41e95,
            &[
                0x48, 0x8b, 0x06, 0x49, 0x8d, 0x55, 0xe8, 0x48, 0x8b, 0xce, 0xff, 0x50, 0x30,
            ][..],
        ),
    ] {
        if read(false, rva, expected.len()).as_deref() != Some(expected) {
            return Err("scheduler typed caller differs");
        }
    }
    Ok(())
}
#[cfg(test)]
pub(super) fn scope() -> serde_json::Value {
    serde_json::json!({
     "schema":"native-scheduler-publication-partial-v1",
     "host_sha256":"d1a0cfd439e7451be515f47595cfa0b6b983f56d1faf4ea434a4ce3b21491ebe",
     "core_sha256":"092abfbb2176f8df04b5212f819dc269dd83aea866ff05aaf489904a1ff7c41f",
     "core_manager":{"push":0x4a1100,"set_offset":0x400,"list_offset":0x410,"spawn":0x4a10b0,"shutdown":0x4a1150,"native_wait":0x9922c0,"free_list":0x9921d0},
     "new_frames":[0x9858b0,0x985f40,0x986010,0xe3e8c0,0xe42470,0xe427f0,0x985a80,0x985bd0],
     "birth":{"normal_allocator_return_add":0x985904,"normal_allocator_return_push":0x985f9e,"existing_wrapper_return":0x986497,"authority":"existing same-thread NativeFrames hold or bound job only; no raw-address/current-world adoption"},
     "cancel":"DestroyJobSet/FreeList entry or normal return alone settles no T; only original T destructor normal return settles a recorded T id",
     "copies":"tagged proxies are scheduler-owned representations of a retained T; this slice retains full dispatcher frames and does not certify all proxy aliases",
     "not_established_by_this_audit":["natural admission seal across all producers","unknown asynchronous publisher provenance","all direct and indirect caller closure","all copied/tagged T aliases and cancel/reissue exactly-once","same-set native Wait across every scheduler queue","state3 as producer seal","fullcoverage or product acceptance"],
     "certificate_issued":false,"product_acceptance":false
    })
}
pub(super) fn core_frames() -> &'static [FixedFrame] {
    core::FRAMES
}
