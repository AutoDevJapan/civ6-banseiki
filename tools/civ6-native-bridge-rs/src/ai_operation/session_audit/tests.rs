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
fn all_frames(dll: bool) -> Vec<&'static Frame> {
    let mut result: Vec<_> = frames(dll).iter().collect();
    if !dll {
        result.extend(lease::fixed::FRAMES);
        result.extend(snapshot::fixed::FRAMES);
        result.extend(view_worker::fixed::FRAMES);
    } else {
        result.extend(world::fixed::FRAMES);
    }
    result
}
fn all_slots(dll: bool) -> Vec<(u32, u32)> {
    let mut result = slots(dll).to_vec();
    result.extend(if dll {
        world::flow::SLOTS
    } else {
        lease::flow::SLOTS
    });
    result.sort_unstable();
    if !dll {
        result.extend(snapshot::flow::SLOTS);
        result.extend(view_worker::flow::SLOTS);
        result.sort_unstable();
    }
    result.dedup();
    result
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
    let mut rows: Vec<_> = all_frames(dll).iter().flat_map(|f| f.fragments).collect();
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
    for f in all_frames(dll) {
        write_hex(&mut b, f.begin, f.specimen, dll);
    }
    for v in blocks(dll) {
        write_hex(&mut b, v.rva, v.hex, dll);
    }
    for (slot, target) in all_slots(dll) {
        w64(&mut b, offset(slot, dll), base + u64::from(target));
    }
    if dll {
        for block in world::flow::BLOCKS {
            write_hex(&mut b, block.rva, block.hex, true);
        }
        for (rva, value) in [
            (0xa18d90, &b"UnitManager\0"[..]),
            (0xa45238, &b"Notifications\0"[..]),
            (0x9c4860, &b"ObjectPairing\0"[..]),
        ] {
            let p = offset(rva, true);
            b[p..p + value.len()].copy_from_slice(value);
        }
    }
    if !dll {
        for v in view_worker::flow::BLOCKS {
            write_hex(&mut b, v.rva, v.hex, false);
        }
        for v in snapshot::flow::BLOCKS {
            write_hex(&mut b, v.rva, v.hex, false);
        }
        let p = offset(0xfd3cc0, false);
        for (i, value) in [2, 0, 4, 0].into_iter().enumerate() {
            w32(&mut b, p + i * 4, value);
        }
        for v in lease::flow::BLOCKS {
            write_hex(&mut b, v.rva, v.hex, false);
        }
        for &(slot, target) in lease::flow::SLOTS {
            w64(&mut b, offset(slot, false), base + u64::from(target));
        }
        // Named imports are independent witnesses of the indirect API calls.
        w32(&mut b, opt + 112 + 8, 0x1100000);
        w32(&mut b, offset(0x1100000, false), 0x1100100);
        w32(&mut b, offset(0x110000c, false), 0x1100200);
        w32(&mut b, offset(0x1100010, false), 0xf97750);
        for (rva, value) in [
            (0x1100200, &b"KERNEL32.dll\0"[..]),
            (0x1100302, &b"WaitForSingleObject\0"[..]),
            (0x1100342, &b"GetLastError\0"[..]),
        ] {
            let p = offset(rva, false);
            b[p..p + value.len()].copy_from_slice(value);
        }
        w64(&mut b, offset(0x1100100, false), 0x1100300);
        w32(&mut b, offset(0x1100014, false), 0x1100140);
        w32(&mut b, offset(0x1100020, false), 0x1100200);
        w32(&mut b, offset(0x1100024, false), 0xf979e8);
        w64(&mut b, offset(0x1100140, false), 0x1100340);
        for (i, (cell, name)) in [
            (0xf977d0, "TlsGetValue"),
            (0xf97840, "CreateEventW"),
            (0xf97800, "WaitForSingleObjectEx"),
            (0xf97970, "CloseHandle"),
            (0xf97740, "SwitchToThread"),
        ]
        .into_iter()
        .enumerate()
        {
            let row = 0x1100028 + i as u32 * 20;
            let lookup = 0x1100180 + i as u32 * 16;
            let hint = 0x1100400 + i as u32 * 64;
            w32(&mut b, offset(row, false), lookup);
            w32(&mut b, offset(row + 12, false), 0x1100200);
            w32(&mut b, offset(row + 16, false), cell);
            w64(&mut b, offset(lookup, false), u64::from(hint));
            let p = offset(hint + 2, false);
            b[p..p + name.len()].copy_from_slice(name.as_bytes());
        }
    }
    if dll {
        let mut groups = std::collections::BTreeMap::<u32, Vec<u32>>::new();
        for (slot, _) in all_slots(true) {
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
    let report = inspect_image(
        &parse(b, dll)?,
        frames(dll),
        blocks(dll),
        refs(dll),
        slots(dll),
        dll,
    )?;
    if !dll {
        lease::inspect(&parse(b, dll)?)?;
        snapshot::inspect(&parse(b, dll)?)?;
        view_worker::inspect(&parse(b, dll)?)?;
    } else {
        world::inspect(&parse(b, dll)?)?;
    }
    Ok(report)
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
fn session_native_lease_contract_requires_existing_birth_and_successful_native_join() {
    let report = lease::inspect(&Pe::parse_host(&specimen(false)).unwrap()).unwrap();
    assert_eq!(report["registration"]["factory_return"], 0x7f8cf5);
    assert_eq!(report["registration"]["owner_publish"], 0x7f8d06);
    assert_eq!(report["registration"]["registry_publish"], 0x7f8d6c);
    assert_eq!(report["connection"]["table"], 0x10494a8);
    assert_eq!(report["dispatch"]["entry"], 0x842030);
    assert_eq!(report["local_thread_join"]["join_return"], 0x8573c9);
    assert_eq!(
        report["local_thread_join"]["wait_api"],
        "KERNEL32.dll!WaitForSingleObject"
    );
    assert_eq!(report["certificate_issued"], false);
}

#[test]
fn session_native_lease_body_unwind_and_table_mutations_reject() {
    let mut host = specimen(false);
    lease::inspect(&Pe::parse_host(&host).unwrap()).unwrap();
    for f in lease::fixed::FRAMES {
        for rva in [f.begin, f.begin + (f.end - f.begin) / 2, f.end - 1] {
            let p = offset(rva, false);
            host[p] ^= 1;
            assert!(
                lease::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
                "lease frame {rva:x}"
            );
            host[p] ^= 1;
        }
        for part in f.fragments {
            let p = offset(part.unwind, false);
            host[p] ^= 1;
            assert!(
                lease::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
                "lease unwind {:x}",
                part.unwind
            );
            host[p] ^= 1;
        }
    }
    for &(slot, _) in lease::flow::SLOTS {
        let p = offset(slot, false);
        host[p] ^= 1;
        assert!(
            lease::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
            "lease slot {slot:x}"
        );
        host[p] ^= 1;
    }
}

#[test]
fn session_native_lease_publication_receiver_stop_and_join_branch_mutations_reject() {
    let mut host = specimen(false);
    for b in lease::flow::BLOCKS {
        for i in 0..b.hex.len() / 2 {
            let p = offset(b.rva, false) + i;
            host[p] ^= 1;
            assert!(
                lease::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
                "lease flow {:x}+{i}",
                b.rva
            );
            host[p] ^= 1;
        }
    }
    for r in lease::flow::REFERENCES {
        let p = offset(r.rva, false) + r.displacement;
        host[p] ^= 1;
        assert!(
            references(&Pe::parse_host(&host).unwrap(), lease::flow::REFERENCES).is_err(),
            "lease ref {:x}",
            r.rva
        );
        host[p] ^= 1;
    }
    lease::inspect(&Pe::parse_host(&host).unwrap()).unwrap();
}

#[test]
fn session_native_join_named_api_rejects_wrong_import_thunk_dll_and_name() {
    let mut host = specimen(false);
    let named = |b: &[u8]| {
        Pe::parse_host(b)
            .unwrap()
            .import(0xf97750, "KERNEL32.dll", "WaitForSingleObject")
    };
    named(&host).unwrap();
    for rva in [
        0x1100000, 0x110000c, 0x1100010, 0x1100100, 0x1100200, 0x1100302,
    ] {
        let p = offset(rva, false);
        host[p] ^= 1;
        assert!(named(&host).is_err(), "named import {rva:x}");
        host[p] ^= 1;
    }
    let p = offset(0x1100100, false);
    host[p + 7] = 0x80;
    assert!(named(&host).is_err(), "ordinal thunk");
    host[p + 7] = 0;
    w32(&mut host, 0x98 + 112 + 8, 0);
    assert!(named(&host).is_err(), "missing import directory");
}

#[test]
fn session_world_records_trace_registered_types_and_distinguish_native_id_domains() {
    let b = specimen(true);
    let report = world::inspect(&Pe::parse(&b).unwrap()).unwrap();
    assert_eq!(
        report["world_registration"]["registration_entry_shared"],
        0x63da80
    );
    assert_eq!(report["materialization"]["bind_entry"], 0x987b0);
    assert_eq!(report["materialization"]["bind_caller_return"], 0x63ccb2);
    assert_eq!(report["materialization"]["record_stride"], 0x120);
    assert_eq!(report["object_pairing_chain"]["object_table"], 0xa22c98);
    assert_eq!(
        report["object_pairing_chain"]["typed_shutdown_target"],
        0x641240
    );
    assert_eq!(report["shutdown"]["vcall"], 0x50358);
    assert_eq!(report["certificate_issued"], false);
    assert_ne!(
        report["object_pairing_chain"]["record_index_global"],
        0x235b874
    );
}

#[test]
fn session_world_registration_and_move_semantics_reject_every_witness_byte_mutation() {
    let mut b = specimen(true);
    world::inspect(&Pe::parse(&b).unwrap()).unwrap();
    for block in world::flow::BLOCKS {
        for i in 0..block.hex.len() / 2 {
            let p = offset(block.rva, true) + i;
            b[p] ^= 1;
            assert!(
                world::inspect(&Pe::parse(&b).unwrap()).is_err(),
                "World block {:x}+{i}",
                block.rva
            );
            b[p] ^= 1;
        }
    }
    for r in world::flow::REFERENCES {
        let p = offset(r.rva, true) + r.displacement;
        b[p] ^= 1;
        assert!(
            references(&Pe::parse(&b).unwrap(), world::flow::REFERENCES).is_err(),
            "World ref {:x}",
            r.rva
        );
        b[p] ^= 1;
    }
}

#[test]
fn session_world_record_tables_names_and_complete_callback_frames_reject_mutation() {
    let mut b = specimen(true);
    for f in world::fixed::FRAMES {
        for rva in [f.begin, f.begin + (f.end - f.begin) / 2, f.end - 1] {
            let p = offset(rva, true);
            b[p] ^= 1;
            assert!(
                world::inspect(&Pe::parse(&b).unwrap()).is_err(),
                "World frame {rva:x}"
            );
            b[p] ^= 1;
        }
    }
    for &(slot, _) in world::flow::SLOTS {
        let p = offset(slot, true);
        b[p] ^= 1;
        assert!(
            world::inspect(&Pe::parse(&b).unwrap()).is_err(),
            "World slot {slot:x}"
        );
        b[p] ^= 1;
    }
    for rva in [0xa18d90, 0xa45238, 0x9c4860] {
        let p = offset(rva, true);
        b[p] ^= 1;
        assert!(
            world::inspect(&Pe::parse(&b).unwrap()).is_err(),
            "World record name {rva:x}"
        );
        b[p] ^= 1;
    }
    world::inspect(&Pe::parse(&b).unwrap()).unwrap();
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
        let mut rows: Vec<_> = all_frames(dll).iter().flat_map(|f| f.fragments).collect();
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

#[test]
fn snapshot_preserves_request_outcome_and_existing_message_before_deferred_bytes() {
    let host = specimen(false);
    let r = snapshot::inspect(&Pe::parse_host(&host).unwrap()).unwrap();
    assert_eq!(r["snapshot_admission"]["request_return"], 0x16c938);
    assert_eq!(r["accepted_request"]["busy_reject"], 0x16bd74);
    assert_eq!(r["accepted_request"]["possible_producer_call"], 0x16bd68);
    assert_eq!(r["accepted_request"]["success_set"], 0x16bd6d);
    assert_eq!(r["owned_stream"]["object_publish"], 0x166bda);
    assert_eq!(r["owned_stream"]["copy_call"], 0x166c4b);
    assert_eq!(r["owned_stream"]["selection_entry"], 0x16d280);
    assert_eq!(r["deferred_queue"]["move_normal_return"], 0x8475b2);
    assert_eq!(r["deferred_queue"]["count_publish"], 0x8475c5);
    assert_eq!(r["deferred_queue"]["decode_call"], 0x83d819);
    assert_eq!(r["deferred_queue"]["byte_free"], 0x83d850);
    assert_eq!(r["deferred_queue"]["layout"]["session_queue"], 0xd8);
    assert_eq!(r["certificate_issued"], false);
}

#[test]
fn snapshot_complete_bodies_and_all_unwind_partitions_reject_mutation() {
    let mut host = specimen(false);
    snapshot::inspect(&Pe::parse_host(&host).unwrap()).unwrap();
    for f in snapshot::fixed::FRAMES {
        for rva in [f.begin, f.begin + (f.end - f.begin) / 2, f.end - 1] {
            let p = offset(rva, false);
            host[p] ^= 1;
            assert!(
                snapshot::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
                "frame {rva:x}"
            );
            host[p] ^= 1;
        }
        for part in f.fragments {
            for i in 0..part.hex.len() / 2 {
                let p = offset(part.unwind, false) + i;
                host[p] ^= 1;
                assert!(
                    snapshot::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
                    "unwind {:x}+{i}",
                    part.unwind
                );
                host[p] ^= 1;
            }
        }
    }
    snapshot::inspect(&Pe::parse_host(&host).unwrap()).unwrap();
}

#[test]
fn snapshot_busy_branch_copy_move_and_decode_references_are_independently_decoded() {
    let mut host = specimen(false);
    references(&Pe::parse_host(&host).unwrap(), snapshot::flow::REFERENCES).unwrap();
    for r in snapshot::flow::REFERENCES {
        for i in (0..r.prefix.len() / 2).chain(r.displacement..r.length) {
            let p = offset(r.rva, false) + i;
            host[p] ^= 1;
            assert!(
                references(&Pe::parse_host(&host).unwrap(), snapshot::flow::REFERENCES).is_err(),
                "reference {:x}+{i}",
                r.rva
            );
            host[p] ^= 1;
        }
    }
    // Invert an original busy/reject branch while preserving its target.
    for rva in [0x273f7b, 0x16b5f6, 0x16b7da, 0x16bd66, 0x842450, 0x84245b] {
        let p = offset(rva, false);
        host[p] ^= 1;
        assert!(
            snapshot::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
            "branch {rva:x}"
        );
        host[p] ^= 1;
    }
}

#[test]
fn snapshot_queue_receiver_and_scalar_move_cannot_be_reinterpreted_as_fresh_receive() {
    let mut host = specimen(false);
    snapshot::queue_layout(&Pe::parse_host(&host).unwrap()).unwrap();
    // A different queue with the same element count does not establish identity.
    for (rva, replacement) in [
        (0x842574, 0xa8),
        (0x83d7d2, 0xa8),
        (0x83d82b, 0xe8),
        (0x83d821, 0xe0),
        (0x83d7f1, 0xf8),
        (0x83d858, 0xf0),
    ] {
        let p = offset(rva, false);
        let old = host[p..p + 4].to_vec();
        w32(&mut host, p, replacement);
        assert!(
            snapshot::queue_layout(&Pe::parse_host(&host).unwrap()).is_err(),
            "receiver {rva:x}"
        );
        host[p..p + 4].copy_from_slice(&old);
    }
    for rva in [
        0x52747d, 0x527480, 0x52748b, 0x52749b, 0x8475b2, 0x8475b8, 0x8475be, 0x8475c5, 0x83d7f5,
        0x83d87a,
    ] {
        let p = offset(rva, false);
        host[p] ^= 1;
        assert!(
            snapshot::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
            "ownership {rva:x}"
        );
        host[p] ^= 1;
    }
}

#[test]
fn snapshot_descriptor_leaf_and_independent_owned_stream_global_mutations_reject() {
    let mut host = specimen(false);
    for rva in [
        0x168b90, 0x168b97, 0x168b99, 0x168ba0, 0x168ba5, 0x168bba, 0xfd3cc0, 0xfd3cc8, 0xfd36b0,
        0x16d280, 0x16d285, 0x16d297, 0x16d29b, 0x16d2a1, 0x16d2a7, 0x166bda, 0x166c11, 0x166c4e,
    ] {
        let p = offset(rva, false);
        host[p] ^= 1;
        assert!(
            snapshot::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
            "snapshot identity {rva:x}"
        );
        host[p] ^= 1;
    }
    // Preserve a well-formed RIP load while selecting a different global stream.
    let p = offset(0x16d285 + 3, false);
    let old = host[p..p + 4].to_vec();
    w32(&mut host, p, (0x22d8d20_i64 - (0x16d285 + 7)) as u32);
    assert!(references(&Pe::parse_host(&host).unwrap(), snapshot::flow::REFERENCES).is_err());
    host[p..p + 4].copy_from_slice(&old);
    snapshot::inspect(&Pe::parse_host(&host).unwrap()).unwrap();
}

#[test]
fn view_worker_same_heap_requires_task_birth_and_classified_group_wait() {
    let host = specimen(false);
    let r = view_worker::inspect(&Pe::parse_host(&host).unwrap()).unwrap();
    assert_eq!(r["layout"]["payload_in_view"], 0x248);
    assert_eq!(r["layout"]["set_in_view"], 0x278);
    assert_eq!(r["layout"]["active_group_in_view"], 0x280);
    assert_eq!(r["layout"]["saved_owner_in_view"], 0x290);
    assert_eq!(r["layout"]["saved_callback_in_view"], 0x298);
    assert_eq!(r["payload_type"]["payload_table"], 0x106c390);
    assert_eq!(r["payload_type"]["payload_execute"], 0x985b50);
    assert_eq!(r["payload_type"]["state_execute"], 0x83450);
    assert_eq!(r["publication"]["allocate_return"], 0x985904);
    assert_eq!(r["publication"]["task_payload_store"], 0x98591a);
    assert_eq!(r["publication"]["publish_tail"], 0x985962);
    assert_eq!(r["execution"]["worker"], 0x82da0);
    assert_eq!(r["execution"]["tls_api"], "KERNEL32.dll!TlsGetValue");
    assert_eq!(r["finish_and_join"]["group_wait_return"], 0x985bb0);
    assert_eq!(r["finish_and_join"]["early_return_epilogue"], 0xe3ee30);
    assert_eq!(r["certificate_issued"], false);
}

#[test]
fn view_worker_complete_prolog_body_epilogue_and_unwind_mutations_reject() {
    let mut host = specimen(false);
    view_worker::inspect(&Pe::parse_host(&host).unwrap()).unwrap();
    for f in view_worker::fixed::FRAMES {
        for rva in [f.begin, f.begin + (f.end - f.begin) / 2, f.end - 1] {
            let p = offset(rva, false);
            host[p] ^= 1;
            assert!(
                view_worker::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
                "View frame {rva:x}"
            );
            host[p] ^= 1;
        }
        for part in f.fragments {
            for i in 0..part.hex.len() / 2 {
                let p = offset(part.unwind, false) + i;
                host[p] ^= 1;
                assert!(
                    view_worker::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
                    "View unwind {:x}+{i}",
                    part.unwind
                );
                host[p] ^= 1;
            }
        }
    }
    view_worker::inspect(&Pe::parse_host(&host).unwrap()).unwrap();
}

#[test]
fn view_worker_state_payload_and_group_fields_have_independent_identity() {
    let mut host = specimen(false);
    view_worker::layout(&Pe::parse_host(&host).unwrap()).unwrap();
    for rva in [
        0x8313b, 0x98617a, 0x98616a, 0x985b5c, 0x83456, 0x8345a, 0x985b8d,
    ] {
        let p = offset(rva, false);
        host[p] ^= 8;
        assert!(
            view_worker::layout(&Pe::parse_host(&host).unwrap()).is_err(),
            "View field {rva:x}"
        );
        host[p] ^= 8;
    }
    for &(slot, _) in view_worker::flow::SLOTS {
        let p = offset(slot, false);
        host[p] ^= 1;
        assert!(
            view_worker::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
            "View type {slot:x}"
        );
        host[p] ^= 1;
    }
}

#[test]
fn view_worker_prepublication_callback_and_early_wait_branches_reject_mutation() {
    let mut host = specimen(false);
    references(
        &Pe::parse_host(&host).unwrap(),
        view_worker::flow::REFERENCES,
    )
    .unwrap();
    for r in view_worker::flow::REFERENCES {
        for i in (0..r.prefix.len() / 2).chain(r.displacement..r.length) {
            let p = offset(r.rva, false) + i;
            host[p] ^= 1;
            assert!(
                references(
                    &Pe::parse_host(&host).unwrap(),
                    view_worker::flow::REFERENCES
                )
                .is_err(),
                "View ref {:x}+{i}",
                r.rva
            );
            host[p] ^= 1;
        }
    }
    // Same targets with inverted skip/early-return conditions must not certify.
    for rva in [
        0x8310e, 0x88d77, 0x985b94, 0xe3eb2f, 0xe3eb3a, 0xe3eb49, 0xe3ee0b,
    ] {
        let p = offset(rva, false);
        host[p] ^= 1;
        assert!(
            view_worker::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
            "View branch {rva:x}"
        );
        host[p] ^= 1;
    }
    for rva in [
        0x98591a, 0x98592b, 0x98592f, 0x9862ef, 0x9862f7, 0x98630b, 0x985b60, 0x985b69, 0x83457,
        0x985bb0,
    ] {
        let p = offset(rva, false);
        host[p] ^= 1;
        assert!(
            view_worker::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
            "View carrier {rva:x}"
        );
        host[p] ^= 1;
    }
}

#[test]
fn view_worker_tls_and_group_event_wait_named_imports_are_distinct() {
    let mut host = specimen(false);
    view_worker::inspect(&Pe::parse_host(&host).unwrap()).unwrap();
    // Switch TLS, event wait, and close to another otherwise valid named API.
    for (i, _) in [
        "TlsGetValue",
        "CreateEventW",
        "WaitForSingleObjectEx",
        "CloseHandle",
        "SwitchToThread",
    ]
    .iter()
    .enumerate()
    {
        let p = offset(0x1100180 + i as u32 * 16, false);
        let old = host[p..p + 8].to_vec();
        w64(&mut host, p, 0x1100300);
        assert!(
            view_worker::inspect(&Pe::parse_host(&host).unwrap()).is_err(),
            "View import {i}"
        );
        host[p..p + 8].copy_from_slice(&old);
    }
    view_worker::inspect(&Pe::parse_host(&host).unwrap()).unwrap();
}
