use super::*;

fn w16(b: &mut [u8], p: usize, v: u16) {
    b[p..p + 2].copy_from_slice(&v.to_le_bytes());
}
fn w32(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn w64(b: &mut [u8], p: usize, v: u64) {
    b[p..p + 8].copy_from_slice(&v.to_le_bytes());
}
fn offset(r: u32, dll: bool) -> usize {
    if dll && r >= 0x9b7000 {
        0x9b5c00 + (r - 0x9b7000) as usize
    } else {
        r as usize - 0xc00
    }
}
fn write_hex(b: &mut [u8], r: u32, hex: &str, dll: bool) {
    let code = unhex(hex).unwrap();
    let p = offset(r, dll);
    b[p..p + code.len()].copy_from_slice(&code);
}
fn frames(dll: bool) -> &'static [Frame] {
    if dll {
        fixed::CORE_FRAMES
    } else {
        fixed::HOST_FRAMES
    }
}
fn blocks(dll: bool) -> &'static [Block] {
    if dll {
        flow::CORE_BLOCKS
    } else {
        flow::HOST_BLOCKS
    }
}
fn refs(dll: bool) -> &'static [Reference] {
    if dll {
        flow::CORE_REFERENCES
    } else {
        flow::HOST_REFERENCES
    }
}
fn slots(dll: bool) -> &'static [(u32, u32)] {
    if dll {
        flow::CORE_SLOTS
    } else {
        flow::HOST_SLOTS
    }
}
const PDATA: usize = 0x3800000;
const RELOC: usize = 0x3810000;
fn specimen(dll: bool) -> Vec<u8> {
    let mut b = vec![0; RELOC + 0x1000];
    b[..2].copy_from_slice(b"MZ");
    w32(&mut b, 0x3c, 0x80);
    b[0x80..0x84].copy_from_slice(b"PE\0\0");
    w16(&mut b, 0x84, 0x8664);
    w16(&mut b, 0x86, 4);
    w16(&mut b, 0x94, 0xf0);
    w16(&mut b, 0x96, if dll { 0x2002 } else { 2 });
    let opt = 0x98;
    w16(&mut b, opt, 0x20b);
    let base = if dll { 0x180000000 } else { 0x140000000 };
    w64(&mut b, opt + 24, base);
    w32(&mut b, opt + 108, 16);
    let mut rows: Vec<_> = frames(dll).iter().flat_map(|f| f.fragments).collect();
    rows.sort_by_key(|f| f.begin);
    w32(&mut b, opt + 112 + 3 * 8, PDATA as u32);
    w32(&mut b, opt + 116 + 3 * 8, (rows.len() * 12) as u32);
    let (data_rva, data_raw, text_len, data_len) = if dll {
        (0x9b7000, 0x9b5c00, 0x9b6000, 0x200000)
    } else {
        (0xf96000, 0xf95400, 0xf95000, 0x2600000)
    };
    for (i, (name, rva, raw, len, flags)) in [
        (b".text\0\0\0", 0x1000, 0x400, text_len, 0x60000020),
        (b".rdata\0\0", data_rva, data_raw, data_len, 0x40000040),
        (b".pdata\0\0", PDATA, PDATA, 0x1000, 0x40000040),
        (b".reloc\0\0", RELOC, RELOC, 0x1000, 0x42000040),
    ]
    .into_iter()
    .enumerate()
    {
        let p = opt + 0xf0 + i * 40;
        b[p..p + 8].copy_from_slice(name);
        for (d, v) in [(8, len), (12, rva), (16, len), (20, raw), (36, flags)] {
            w32(&mut b, p + d, v as u32);
        }
    }
    for (i, f) in rows.into_iter().enumerate() {
        w32(&mut b, PDATA + i * 12, f.begin);
        w32(&mut b, PDATA + i * 12 + 4, f.end);
        w32(&mut b, PDATA + i * 12 + 8, f.unwind);
        write_hex(&mut b, f.unwind, f.hex, dll);
    }
    for f in frames(dll) {
        write_hex(&mut b, f.begin, f.specimen, dll);
    }
    for v in blocks(dll) {
        write_hex(&mut b, v.rva, v.hex, dll);
    }
    for &(slot, target) in slots(dll) {
        w64(&mut b, offset(slot, dll), base + u64::from(target));
    }
    if dll {
        let mut groups = std::collections::BTreeMap::<u32, Vec<u32>>::new();
        for &(slot, _) in slots(true) {
            groups.entry(slot & !0xfff).or_default().push(slot);
        }
        let mut p = RELOC;
        for (page, group) in groups {
            let count = (group.len() + 1) & !1;
            w32(&mut b, p, page);
            w32(&mut b, p + 4, (8 + count * 2) as u32);
            for (i, slot) in group.iter().enumerate() {
                w16(&mut b, p + 8 + i * 2, 0xa000 | (slot & 0xfff) as u16);
            }
            p += 8 + count * 2;
        }
        w32(&mut b, opt + 112 + 5 * 8, RELOC as u32);
        w32(&mut b, opt + 116 + 5 * 8, (p - RELOC) as u32);
    }
    b
}
fn parse(b: &[u8], dll: bool) -> Result<Pe<'_>, String> {
    if dll {
        Pe::parse(b)
    } else {
        Pe::parse_host(b)
    }
}
fn valid(b: &[u8], dll: bool) -> Result<Value, String> {
    inspect_image(
        &parse(b, dll)?,
        frames(dll),
        blocks(dll),
        refs(dll),
        slots(dll),
        dll,
    )
}

#[test]
fn session_complete_synthetic_pair_exercises_message_context_and_shutdown() {
    let core = specimen(true);
    let host = specimen(false);
    let report = inspect(&Pe::parse(&core).unwrap(), &Pe::parse_host(&host).unwrap()).unwrap();
    assert_eq!(report["context_admission"]["enqueue"]["slot"], 0x230);
    assert_eq!(report["message_lifetime"]["dispatch"], 0x265660);
    assert_eq!(
        report["message_lifetime"]["current_context_lookup"]["global"],
        0x22d70c0
    );
    assert_eq!(report["shutdown_order"]["fixed_target"], 0x502e0);
    assert_eq!(report["shutdown_order"]["first_destructive_use"], 0x380e3);
    assert_eq!(report["certificate_issued"], false);
    assert!(report["not_established_by_this_audit"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str().unwrap().contains("old packet/session identity")));
}

#[test]
fn session_full_frames_reject_body_and_epilogue_mutations() {
    for dll in [true, false] {
        let mut b = specimen(dll);
        valid(&b, dll).unwrap();
        for f in frames(dll) {
            for r in [f.begin, f.begin + (f.end - f.begin) / 2, f.end - 1] {
                let p = offset(r, dll);
                b[p] ^= 1;
                assert!(valid(&b, dll).is_err(), "whole frame {r:x}");
                b[p] ^= 1;
            }
        }
        valid(&b, dll).unwrap();
    }
}

#[test]
fn session_all_pdata_and_chain_unwind_fields_are_required() {
    for dll in [true, false] {
        let mut b = specimen(dll);
        valid(&b, dll).unwrap();
        let mut rows: Vec<_> = frames(dll).iter().flat_map(|f| f.fragments).collect();
        rows.sort_by_key(|f| f.begin);
        for (i, f) in rows.into_iter().enumerate() {
            for p in [PDATA + i * 12, PDATA + i * 12 + 4, PDATA + i * 12 + 8] {
                b[p] ^= 1;
                assert!(valid(&b, dll).is_err(), "pdata {p:x}");
                b[p] ^= 1;
            }
            for i in 0..f.hex.len() / 2 {
                let p = offset(f.unwind, dll) + i;
                b[p] ^= 1;
                assert!(valid(&b, dll).is_err(), "unwind {:x}+{i}", f.unwind);
                b[p] ^= 1;
            }
        }
        valid(&b, dll).unwrap();
    }
}

#[test]
fn session_field_flow_vcall_and_gateway_prefix_bytes_reject_mutations() {
    for dll in [true, false] {
        let mut b = specimen(dll);
        let p = parse(&b, dll).unwrap();
        for v in blocks(dll) {
            exact(&p, v.rva, v.hex).unwrap();
        }
        for v in blocks(dll) {
            for i in 0..v.hex.len() / 2 {
                let pos = offset(v.rva, dll) + i;
                b[pos] ^= 1;
                assert!(
                    exact(&parse(&b, dll).unwrap(), v.rva, v.hex).is_err(),
                    "flow {:x}+{i}",
                    v.rva
                );
                b[pos] ^= 1;
            }
        }
    }
}

#[test]
fn session_relative_rip_branch_directions_and_bounds_are_independent() {
    for dll in [true, false] {
        let mut b = specimen(dll);
        references(&parse(&b, dll).unwrap(), refs(dll)).unwrap();
        for r in refs(dll) {
            for i in (0..r.prefix.len() / 2).chain(r.displacement..r.length) {
                let p = offset(r.rva, dll) + i;
                b[p] ^= 1;
                assert!(
                    references(&parse(&b, dll).unwrap(), refs(dll)).is_err(),
                    "reference {:x}+{i}",
                    r.rva
                );
                b[p] ^= 1;
            }
        }
    }
    assert_eq!(target(0x100, &[0x75, 0xf0], 2, 1).unwrap(), 0xf2);
    assert_eq!(
        target(0x100, &[0xe9, 0xf0, 0xff, 0xff, 0xff], 5, 1).unwrap(),
        0xf5
    );
    assert!(target(0, &[0x75, 0x80], 2, 1).is_err());
    assert!(target(u32::MAX, &[0x75, 0], 2, 1).is_err());
    assert!(target(0, &[0xe8, 0], 5, 1).is_err());
}

#[test]
fn session_vtable_pointer_relocation_code_data_and_truncation_reject() {
    for dll in [true, false] {
        let mut b = specimen(dll);
        valid(&b, dll).unwrap();
        for &(slot, _) in slots(dll) {
            let p = offset(slot, dll);
            b[p] ^= 1;
            assert!(valid(&b, dll).is_err(), "slot {slot:x}");
            b[p] ^= 1;
        }
        let section_table = 0x98 + 0xf0;
        // Remove execute from .text using the high characteristics byte.
        b[section_table + 39] ^= 0x20;
        assert!(valid(&b, dll).is_err());
        b[section_table + 39] ^= 0x20;
        b[section_table + 40 + 39] ^= 0x20;
        assert!(valid(&b, dll).is_err());
        b[section_table + 40 + 39] ^= 0x20;
        assert!(valid(&b[..PDATA + 5], dll).is_err());
        if dll {
            b[RELOC + 8] ^= 1;
            assert!(valid(&b, true).is_err());
        }
    }
    let mut host = specimen(false);
    assert!(Pe::parse(&host).is_err());
    w32(&mut host, 0x98 + 112 + 5 * 8, RELOC as u32);
    assert!(
        Pe::parse_host(&host).is_err(),
        "one-sided relocation directory"
    );
}

#[test]
fn session_rejects_unpinned_files_before_parsing_either_abi() {
    assert!(audit(b"MZ", b"PE").unwrap_err().contains("pinned"));
    assert!(pair(&[], &[]).is_err());
}

#[test]
fn session_global_addresses_allow_zero_filled_data_but_reject_code_or_outside_image() {
    let mut b = specimen(false);
    let data_section = 0x98 + 0xf0 + 40;
    // Back only the table/unwind interval; later globals are loader-zero-filled.
    w32(&mut b, data_section + 16, 0x300000);
    let p = Pe::parse_host(&b).unwrap();
    p.virtual_data_address(0x22d70c0, 8).unwrap();
    assert!(p.offset(0x22d70c0, 8).is_err());
    assert!(p.virtual_data_address(0x265660, 8).is_err());
    assert!(p.virtual_data_address(u32::MAX, 8).is_err());
    assert!(p.virtual_data_address(0x3700000, 8).is_err());
    references(&p, flow::HOST_REFERENCES).unwrap();
}
