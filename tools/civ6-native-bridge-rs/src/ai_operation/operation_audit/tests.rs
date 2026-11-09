use super::*;

const TEXT_RVA: u32 = 0x1000;
const TEXT_RAW: usize = 0x400;
const DATA_RVA: u32 = 0x9b7000;
const DATA_RAW: usize = 0x9b5c00;
const PDATA: usize = 0xc00000;
const RELOC: usize = 0xc10000;
fn w16(b: &mut [u8], p: usize, v: u16) {
    b[p..p + 2].copy_from_slice(&v.to_le_bytes());
}
fn w32(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn w64(b: &mut [u8], p: usize, v: u64) {
    b[p..p + 8].copy_from_slice(&v.to_le_bytes());
}
fn offset(rva: u32) -> usize {
    if rva < DATA_RVA {
        TEXT_RAW + (rva - TEXT_RVA) as usize
    } else {
        DATA_RAW + (rva - DATA_RVA) as usize
    }
}
fn code(b: &mut [u8], rva: u32, hex: &str) {
    let v = unhex(hex).unwrap();
    let p = offset(rva);
    b[p..p + v.len()].copy_from_slice(&v);
}
fn specimen() -> Vec<u8> {
    let mut b = vec![0u8; RELOC + 0x1000];
    b[..2].copy_from_slice(b"MZ");
    w32(&mut b, 0x3c, 0x80);
    b[0x80..0x84].copy_from_slice(b"PE\0\0");
    w16(&mut b, 0x84, 0x8664);
    w16(&mut b, 0x86, 4);
    w16(&mut b, 0x94, 0xf0);
    w16(&mut b, 0x96, 0x2002);
    let opt = 0x98;
    w16(&mut b, opt, 0x20b);
    w64(&mut b, opt + 24, 0x180000000);
    w32(&mut b, opt + 108, 16);
    let mut rows: Vec<_> = fixed::FRAMES.iter().flat_map(|f| f.fragments).collect();
    rows.sort_by_key(|f| f.begin);
    w32(&mut b, opt + 112 + 3 * 8, PDATA as u32);
    w32(&mut b, opt + 116 + 3 * 8, (rows.len() * 12) as u32);
    w32(&mut b, opt + 112 + 5 * 8, RELOC as u32);
    w32(&mut b, opt + 116 + 5 * 8, 52);
    for (i, (name, rva, raw, len, flags)) in [
        (b".text\0\0\0", TEXT_RVA, TEXT_RAW, 0x9b6000, 0x60000020),
        (b".rdata\0\0", DATA_RVA, DATA_RAW, 0x1a8124, 0x40000040),
        (b".pdata\0\0", PDATA as u32, PDATA, 0x1000, 0x40000040),
        (b".reloc\0\0", RELOC as u32, RELOC, 0x1000, 0x42000040),
    ]
    .into_iter()
    .enumerate()
    {
        let p = opt + 0xf0 + i * 40;
        b[p..p + 8].copy_from_slice(name);
        w32(&mut b, p + 8, len);
        w32(&mut b, p + 12, rva);
        w32(&mut b, p + 16, len);
        w32(&mut b, p + 20, raw as u32);
        w32(&mut b, p + 36, flags);
    }
    for (i, f) in rows.into_iter().enumerate() {
        w32(&mut b, PDATA + i * 12, f.begin);
        w32(&mut b, PDATA + i * 12 + 4, f.end);
        w32(&mut b, PDATA + i * 12 + 8, f.unwind);
        code(&mut b, f.unwind, f.hex);
    }
    for f in fixed::FRAMES {
        code(&mut b, f.begin, f.specimen);
    }
    for l in fixed::LEAVES {
        code(&mut b, l.rva, l.hex);
    }
    for &(rva, hex) in BLOCKS {
        code(&mut b, rva, hex);
    }
    w32(&mut b, RELOC, 0x9be000);
    w32(&mut b, RELOC + 4, 52);
    for (i, (base, target)) in TABLES
        .iter()
        .flat_map(|(base, targets)| {
            targets
                .iter()
                .enumerate()
                .map(move |(i, t)| (base + i as u32 * 8, *t))
        })
        .enumerate()
    {
        w64(&mut b, offset(base), 0x180000000 + u64::from(target));
        w16(&mut b, RELOC + 8 + i * 2, 0xa000 | (base & 0xfff) as u16);
    }
    b
}
fn valid(b: &[u8]) -> Result<Value, String> {
    inspect(&Pe::parse(b)?)
}

#[test]
fn complete_synthetic_pe_exercises_real_consumer_queue_and_factory_witnesses() {
    let b = specimen();
    let report = valid(&b).unwrap();
    assert_eq!(report["execute_consumer"]["execute_call_rva"], 0x37e64);
    assert_eq!(report["queued_path"]["allocator_return_rva"], 0x3743a);
    assert_eq!(
        report["queued_path"]["publication_precedes_payload_copy"],
        true
    );
    assert_eq!(
        report["allocator_adapter_contract"]["incoming_stack_offsets"],
        json!([0x28, 0x30])
    );
    assert_eq!(
        report["whole_frames"].as_array().unwrap().len(),
        fixed::FRAMES.len()
    );
    assert_eq!(
        report["relocated_vtable_slots"].as_array().unwrap().len(),
        21
    );
    assert_eq!(report["certificate_issued"], false);
}
#[test]
fn complete_frames_reject_intervening_code_and_epilogue_changes() {
    let mut b = specimen();
    assert!(valid(&b).is_ok());
    for f in fixed::FRAMES {
        for rva in [f.begin, f.begin + (f.end - f.begin) / 2, f.end - 1] {
            let p = offset(rva);
            b[p] ^= 1;
            assert!(valid(&b).is_err(), "frame {:#x} mutation {rva:#x}", f.begin);
            b[p] ^= 1;
        }
    }
    assert!(valid(&b).is_ok());
}
#[test]
fn exact_dispatch_publication_and_six_argument_allocator_operands_reject_changes() {
    let mut b = specimen();
    assert!(valid(&b).is_ok());
    for rva in [
        0x37c0c, 0x37c23, 0x33c2f, 0x37e56, 0x37e66, 0x37ede, 0x37f58, 0x37400, 0x37409, 0x37435,
        0x37472, 0x37475, 0x3749b, 0x37509, 0x3752f, 0x37342, 0x37353, 0x37366, 0x37370, 0x590812,
        0x590822, 0x994974, 0x99497f, 0x99498c,
    ] {
        let p = offset(rva);
        b[p] ^= 1;
        assert!(valid(&b).is_err(), "mutation {rva:#x}");
        b[p] ^= 1;
    }
    assert!(valid(&b).is_ok());
}
#[test]
fn vtable_targets_and_each_dir64_relocation_are_required() {
    let mut b = specimen();
    assert!(valid(&b).is_ok());
    for (i, slot) in TABLES
        .iter()
        .flat_map(|(base, targets)| (0..targets.len()).map(move |i| base + i as u32 * 8))
        .enumerate()
    {
        let p = offset(slot);
        b[p] ^= 1;
        assert!(valid(&b).is_err(), "slot {slot:#x}");
        b[p] ^= 1;
        let p = RELOC + 8 + i * 2;
        let old = [b[p], b[p + 1]];
        w16(&mut b, p, 0);
        assert!(valid(&b).is_err(), "relocation for slot {slot:#x}");
        b[p..p + 2].copy_from_slice(&old);
    }
    assert!(valid(&b).is_ok());
}
#[test]
fn pdata_partition_and_every_unwind_record_are_checked() {
    let mut b = specimen();
    assert!(valid(&b).is_ok());
    for f in fixed::FRAMES.iter().flat_map(|f| f.fragments) {
        let p = offset(f.unwind);
        b[p + 1] ^= 1;
        assert!(valid(&b).is_err(), "unwind {:#x}", f.unwind);
        b[p + 1] ^= 1;
    }
    let mut rows: Vec<_> = fixed::FRAMES.iter().flat_map(|f| f.fragments).collect();
    rows.sort_by_key(|f| f.begin);
    for i in 0..rows.len() {
        for field in [0, 4, 8] {
            let p = PDATA + i * 12 + field;
            b[p] ^= 1;
            assert!(valid(&b).is_err(), "pdata row{i} field{field}");
            b[p] ^= 1;
        }
    }
    assert!(valid(&b).is_ok());
}
#[test]
fn relative_operands_are_computed_in_both_directions_and_bounded() {
    let mut b = specimen();
    let pe = Pe::parse(&b).unwrap();
    relative(&pe, 0x37353, 5, 1, 0x37b50).unwrap();
    relative(&pe, 0x590815, 5, 1, 0x373e0).unwrap();
    assert!(relative(&pe, 0x590815, 5, 1, 0x373e1).is_err());
    assert!(relative(&pe, 0x590815, 5, 2, 0x373e0).is_err());
    assert!(pe.span(u32::MAX, 8, true).is_err());
    assert!(pe.span(DATA_RVA, 8, true).is_err());
    w32(&mut b, 0x98 + 0xf0 + 40 + 20, TEXT_RAW as u32);
    assert!(valid(&b).is_err());
}
#[test]
fn public_entry_rejects_unpinned_and_truncated_files_before_abi_interpretation() {
    for b in [Vec::new(), b"MZ".to_vec(), specimen()] {
        assert!(audit(&b).unwrap_err().contains("SHA256"));
    }
    let b = specimen();
    for end in [0, 2, 0x80, 0x84, 0x98, 0x100, PDATA + 11, RELOC + 51] {
        assert!(Pe::parse(&b[..end]).is_err(), "truncation {end:x}");
    }
}
