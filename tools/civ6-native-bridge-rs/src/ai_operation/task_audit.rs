//! Fixed-file evidence for the real JobManager path; no handles or invocation.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const HOST_SHA: &str = "e7450823cc8e00468cff7b9d7b97c63140eae38ae1d774ba4efa437556c42d63";

/// Opt-in slice. The old audit and every value it returns are preserved.
/// Hashes refer to these borrowed file buffers, never a later path reread.
pub fn audit_worker_payload(core: &[u8], host: &[u8]) -> Result<Value, String> {
    let base = audit(core, host)?; // includes BOTH hash gates before interpreting ABI
    let payload = worker_payload::inspect(core)?;
    Ok(json!({"schema":"todo880-native-worker-payload-static-v1",
        "base_audit":base,"payload_slice":payload,
        "structural_declarations":{"process_open_calls":0,"dll_load_calls":0,
            "runtime_feature_activation_calls":0,
            "basis":"file-buffer-only code structure; not measured process or runtime counters"}}))
}

mod worker_payload {
    use super::*;
    // Pure PE section mapping follows civ6-pe-rs/src/main.rs. It additionally
    // checks WHOLE spans and keeps exception-directory unwind RVAs for CHAININFO.
    // No process reader, image loader, CLI decoder or runtime interface here.
    #[derive(Clone)]
    struct Section {
        name: [u8; 8],
        rva: u32,
        virtual_size: u32,
        raw: usize,
        size: usize,
        flags: u32,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Function {
        begin: u32,
        end: u32,
        unwind: u32,
    }
    struct Pe<'a> {
        image: &'a [u8],
        base: u64,
        sections: Vec<Section>,
        functions: Vec<Function>,
        relocations: std::collections::BTreeSet<u32>,
    }
    fn at(b: &[u8], offset: usize, length: usize) -> Result<&[u8], String> {
        b.get(
            offset
                ..offset
                    .checked_add(length)
                    .ok_or("payload file span overflow")?,
        )
        .ok_or("payload file span truncated".into())
    }
    fn u16_at(b: &[u8], p: usize) -> Result<u16, String> {
        Ok(u16::from_le_bytes(at(b, p, 2)?.try_into().unwrap()))
    }
    fn u32_at(b: &[u8], p: usize) -> Result<u32, String> {
        Ok(u32::from_le_bytes(at(b, p, 4)?.try_into().unwrap()))
    }
    fn u64_at(b: &[u8], p: usize) -> Result<u64, String> {
        Ok(u64::from_le_bytes(at(b, p, 8)?.try_into().unwrap()))
    }
    impl<'a> Pe<'a> {
        fn parse(image: &'a [u8]) -> Result<Self, String> {
            if at(image, 0, 2)? != b"MZ" {
                return Err("payload PE MZ signature".into());
            }
            let nt = u32_at(image, 0x3c)? as usize;
            if at(image, nt, 4)? != b"PE\0\0" {
                return Err("payload PE signature".into());
            }
            let coff = nt.checked_add(4).ok_or("payload header overflow")?;
            let count = u16_at(image, coff + 2)? as usize;
            let opt_size = u16_at(image, coff + 16)? as usize;
            let opt = coff + 20;
            if u16_at(image, coff)? != 0x8664
                || count == 0
                || count > 96
                || opt_size < 160
                || u16_at(image, opt)? != 0x20b
                || u32_at(image, opt + 108)? < 6
            {
                return Err("payload PE64 sections/directories".into());
            }
            at(image, opt, opt_size)?;
            let table = opt
                .checked_add(opt_size)
                .ok_or("payload section table overflow")?;
            let mut sections = Vec::new();
            for i in 0..count {
                let p = table + i * 40;
                let section = Section {
                    name: at(image, p, 8)?.try_into().unwrap(),
                    virtual_size: u32_at(image, p + 8)?,
                    rva: u32_at(image, p + 12)?,
                    size: u32_at(image, p + 16)? as usize,
                    raw: u32_at(image, p + 20)? as usize,
                    flags: u32_at(image, p + 36)?,
                };
                section
                    .rva
                    .checked_add(section.virtual_size)
                    .ok_or("payload section RVA overflow")?;
                at(image, section.raw, section.size)?;
                sections.push(section);
            }
            let mut pe = Self {
                image,
                base: u64_at(image, opt + 24)?,
                sections,
                functions: Vec::new(),
                relocations: std::collections::BTreeSet::new(),
            };
            let pdata_rva = u32_at(image, opt + 112 + 3 * 8)?;
            let pdata_size = u32_at(image, opt + 116 + 3 * 8)? as usize;
            if pdata_size == 0 || pdata_size % 12 != 0 || pdata_size / 12 > 131072 {
                return Err("payload exception directory size".into());
            }
            let pdata = pe.span(pdata_rva, pdata_size, false)?;
            for row in pdata.chunks_exact(12) {
                let f = Function {
                    begin: u32_at(row, 0)?,
                    end: u32_at(row, 4)?,
                    unwind: u32_at(row, 8)?,
                };
                if f.begin >= f.end || f.unwind & 3 != 0 {
                    return Err("payload function range/unwind alignment".into());
                }
                pe.span(f.begin, (f.end - f.begin) as usize, true)?;
                pe.span(f.unwind, 4, false)?;
                pe.functions.push(f);
            }
            pe.functions.sort_by_key(|f| f.begin);
            if pe.functions.windows(2).any(|w| w[0].end > w[1].begin) {
                return Err("payload overlapping exception ranges".into());
            }
            let reloc_rva = u32_at(image, opt + 112 + 5 * 8)?;
            let reloc_size = u32_at(image, opt + 116 + 5 * 8)? as usize;
            if reloc_size == 0 {
                return Err("payload relocation directory absent".into());
            }
            let reloc = pe.span(reloc_rva, reloc_size, false)?;
            let mut pos = 0usize;
            while pos < reloc.len() {
                let page = u32_at(reloc, pos)?;
                let size = u32_at(reloc, pos + 4)? as usize;
                if size < 8 || size % 2 != 0 {
                    return Err("payload relocation block size".into());
                }
                let block = at(reloc, pos, size)?;
                for word in block[8..].chunks_exact(2) {
                    let item = u16::from_le_bytes(word.try_into().unwrap());
                    if item >> 12 == 10 {
                        let rva = page
                            .checked_add(u32::from(item & 0xfff))
                            .ok_or("payload relocation overflow")?;
                        pe.span_any(rva, 8)?;
                        pe.relocations.insert(rva);
                    } else if item >> 12 != 0 {
                        return Err("payload non-DIR64 relocation".into());
                    }
                }
                pos = pos
                    .checked_add(size)
                    .ok_or("payload relocation size overflow")?;
            }
            Ok(pe)
        }
        fn section(&self, rva: u32, length: usize) -> Result<&Section, String> {
            let length = u32::try_from(length).map_err(|_| "payload span length")?;
            let end = rva.checked_add(length).ok_or("payload RVA span overflow")?;
            let mut matches = self.sections.iter().filter(|s| {
                rva >= s.rva && end <= s.rva.saturating_add(s.virtual_size.min(s.size as u32))
            });
            let s = matches
                .next()
                .ok_or("payload span outside backed section")?;
            if matches.next().is_some() {
                return Err("payload ambiguous section".into());
            }
            Ok(s)
        }
        fn span_any(&self, rva: u32, length: usize) -> Result<&'a [u8], String> {
            let s = self.section(rva, length)?;
            at(self.image, s.raw + (rva - s.rva) as usize, length)
        }
        fn span(&self, rva: u32, length: usize, code: bool) -> Result<&'a [u8], String> {
            let s = self.section(rva, length)?;
            if (s.flags & 0x2000_0000 != 0) != code || s.flags & 0x4000_0000 == 0 {
                return Err("payload executable/data section type differs".into());
            }
            self.span_any(rva, length)
        }
        fn function(&self, rva: u32) -> Option<Function> {
            let i = self.functions.partition_point(|f| f.begin <= rva);
            i.checked_sub(1)
                .and_then(|i| self.functions.get(i))
                .copied()
                .filter(|f| rva < f.end)
        }
        fn chain(&self, fragment: Function) -> Result<Vec<Function>, String> {
            let mut chain = Vec::new();
            let mut current = fragment;
            for _ in 0..16 {
                if chain.contains(&current) {
                    return Err("payload cyclic CHAININFO".into());
                }
                chain.push(current);
                let h = self.span(current.unwind, 4, false)?;
                if h[0] & 7 != 1 || h[0] >> 3 & !7 != 0 {
                    return Err("payload unwind version/flags".into());
                }
                let flags = h[0] >> 3;
                let codes = usize::from(h[2]);
                let aligned = 4 + ((codes + 1) & !1) * 2;
                self.span(current.unwind, aligned, false)?;
                if flags & 4 == 0 {
                    return Ok(chain);
                }
                if flags & 3 != 0 {
                    return Err("payload chained unwind also has a handler".into());
                }
                let address = current
                    .unwind
                    .checked_add(aligned as u32)
                    .ok_or("payload CHAININFO overflow")?;
                let row = self.span(address, 12, false)?;
                let parent = Function {
                    begin: u32_at(row, 0)?,
                    end: u32_at(row, 4)?,
                    unwind: u32_at(row, 8)?,
                };
                if !self.functions.contains(&parent) {
                    return Err("payload CHAININFO parent absent from pdata".into());
                }
                current = parent;
            }
            Err("payload CHAININFO exceeds finite depth".into())
        }
        fn witness(
            &self,
            rva: u32,
            expected: &[u8],
            whole_function: bool,
        ) -> Result<Value, String> {
            if self.span(rva, expected.len(), true)? != expected {
                return Err(format!("payload bytes differ at {rva:#x}"));
            }
            let f = self.function(rva);
            if whole_function
                && f.is_none_or(|f| f.begin != rva || f.end != rva + expected.len() as u32)
            {
                return Err(format!(
                    "payload complete function range differs at {rva:#x}"
                ));
            }
            let s = self.section(rva, expected.len())?;
            Ok(
                json!({"rva":rva,"length":expected.len(),"section":String::from_utf8_lossy(&s.name).trim_end_matches('\0'),
                "span_type":if whole_function {"complete_pdata_function"} else if f.is_none() {"bounded_leaf_without_pdata"} else {"bounded_instruction_interval"},
                "bytes_hex":expected.iter().map(|b|format!("{b:02x}")).collect::<String>(),
                "function_range":f.map(|f|json!({"begin":f.begin,"end":f.end,"unwind":f.unwind}))}),
            )
        }
        fn pointer(&self, slot: u32, target: u32) -> Result<Value, String> {
            let raw = u64_at(self.span(slot, 8, false)?, 0)?;
            let actual = raw
                .checked_sub(self.base)
                .and_then(|v| u32::try_from(v).ok())
                .ok_or("payload absolute pointer outside image")?;
            if actual != target || !self.relocations.contains(&slot) {
                return Err("payload vft pointer/DIR64 relocation differs".into());
            }
            self.span(target, 1, true)?;
            Ok(
                json!({"slot_rva":slot,"preferred_va":raw,"computed_target_rva":actual,"relocation_type":"DIR64",
                "scope":"relocated disk pointer; not an executed vtable or payload identity"}),
            )
        }
    }
    const GETTER: &[u8] = &[
        0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0x41, 0x08, 0x48, 0x8b, 0x58, 0x60, 0x48,
        0x85, 0xdb, 0x74, 0x2b, 0x48, 0x8b, 0xcb, 0xe8, 0x65, 0xd8, 0x0a, 0, 0x84, 0xc0, 0x74,
        0x1f, 0xb9, 0x20, 0, 0, 0, 0xe8, 0x97, 0xf6, 0, 0, 0x48, 0x85, 0xc0, 0x74, 0x10, 0x48,
        0x8b, 0xd3, 0x48, 0x8b, 0xc8, 0x48, 0x83, 0xc4, 0x20, 0x5b, 0xe9, 0x72, 0x6b, 0, 0, 0x33,
        0xc0, 0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3,
    ];
    const CTOR: &[u8] = &[
        0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x89, 0x51, 0x08, 0x48, 0x8d, 0x05, 0x87, 0xa1,
        0x47, 0, 0x48, 0x89, 0x01, 0x48, 0x8b, 0xd9, 0x48, 0xc7, 0x41, 0x10, 0, 0, 0, 0, 0xc7,
        0x41, 0x18, 1, 0, 0, 0, 0x48, 0x85, 0xd2, 0x74, 0x12, 0x48, 0x8b, 0x0d, 0xfe, 0x3c, 0x5f,
        0, 0x48, 0x85, 0xc9, 0x74, 6, 0x48, 0x8b, 0x01, 0xff, 0x50, 0x40, 0x48, 0x8b, 0xc3, 0x48,
        0x83, 0xc4, 0x20, 0x5b, 0xc3,
    ];
    const OTHER_CTOR: &[u8] = &[
        0x48, 0x8d, 0x05, 0x41, 0xa1, 0x47, 0, 0x48, 0xc7, 0x41, 8, 0, 0, 0, 0, 0x48, 0x89, 1,
        0x48, 0x8b, 0xc1, 0x48, 0x89, 0x51, 0x10, 0xc7, 0x41, 0x18, 1, 0, 0, 0, 0xc3,
    ];
    const COUNTER: &[u8] = &[
        0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x8b, 0x41, 0x18, 0x48, 0x8b, 0xd9, 0x83, 0xf8, 1,
        0x75, 0x30, 0x48, 0x83, 0x79, 8, 0, 0x48, 0x8d, 0x05, 0xfb, 0xa0, 0x47, 0, 0x48, 0x89, 1,
        0x74, 0x12, 0x48, 0x8b, 0x0d, 0x87, 0x3c, 0x5f, 0, 0x48, 0x85, 0xc9, 0x74, 6, 0x48, 0x8b,
        1, 0xff, 0x50, 0x38, 0x48, 0x8b, 0xcb, 0x48, 0x83, 0xc4, 0x20, 0x5b, 0xe9, 0x6f, 0x91,
        0xff, 0xff, 0xff, 0xc8, 0x89, 0x41, 0x18, 0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3,
    ];
    const EXECUTE: &[u8] = &[0x48, 0x8b, 0x49, 8, 0xe9, 0xb7, 0x5b, 0x0a, 0];
    const FINISH: &[u8] = &[0x48, 0x8b, 0x49, 8, 0xe9, 0xc7, 0x67, 0x0a, 0];
    fn rel_target(
        rva: u32,
        code: &[u8],
        opcode: u8,
        length: usize,
        displacement: usize,
    ) -> Result<u32, String> {
        if length != code.len() || code.first() != Some(&opcode) {
            return Err("payload relative opcode/length".into());
        }
        let delta = i32::from_le_bytes(at(code, displacement, 4)?.try_into().unwrap());
        u32::try_from(i64::from(rva) + length as i64 + i64::from(delta))
            .map_err(|_| "payload relative target bounds".into())
    }
    fn computed_branches(pe: &Pe<'_>) -> Result<Vec<Value>, String> {
        let mut result = Vec::new();
        for (rva, op, target) in [
            (0x5901b6, 0xe8, 0x63da20),
            (0x5901c4, 0xe8, 0x59f860),
            (0x5901d9, 0xe9, 0x596d50),
            (0x597634, 0xe9, 0x63d1f0),
            (0x597994, 0xe9, 0x63e160),
            (0x596e0c, 0xe9, 0x58ff80),
        ] {
            let actual = rel_target(rva, pe.span(rva, 5, true)?, op, 5, 1)?;
            if actual != target {
                return Err("payload relative branch target differs".into());
            }
            result.push(
                json!({"rva":rva,"kind":if op==0xe8 {"direct_call"} else {"direct_tail"},
                "computed_target_rva":actual}),
            );
        }
        for (rva, target) in [
            (0x596d5a, 0xa10ee8),
            (0x596da0, 0xa10ee8),
            (0x596de6, 0xa10ee8),
        ] {
            let b = pe.span(rva, 7, true)?;
            if b[..3] != [0x48, 0x8d, 0x05] {
                return Err("payload table LEA opcode".into());
            }
            let actual = rel_target(rva, b, 0x48, 7, 3)?;
            if actual != target {
                return Err("payload table LEA target differs".into());
            }
            result.push(
                json!({"rva":rva,"kind":"rip_relative_address","computed_target_rva":actual}),
            );
        }
        Ok(result)
    }
    fn code(pe: &Pe<'_>) -> Result<Vec<Value>, String> {
        [
            (0x5901a0, GETTER, true),
            (0x596d50, CTOR, true),
            (0x596da0, OTHER_CTOR, false),
            (0x596dd0, COUNTER, true),
            (0x597630, EXECUTE, false),
            (0x597990, FINISH, false),
        ]
        .into_iter()
        .map(|(rva, b, whole)| pe.witness(rva, b, whole))
        .collect()
    }
    pub(super) fn inspect(core: &[u8]) -> Result<Value, String> {
        let pe = Pe::parse(core)?;
        let witnesses = code(&pe)?;
        let branches = computed_branches(&pe)?;
        let pointers = [
            (0xa10278, 0x5901a0),
            (0xa10ee8, 0x5976f0),
            (0xa10ef0, 0x596dd0),
            (0xa10ef8, 0x597630),
            (0xa10f00, 0x597990),
        ]
        .into_iter()
        .map(|(slot, target)| pe.pointer(slot, target))
        .collect::<Result<Vec<_>, _>>()?;
        let fragment = pe
            .function(0x63d443)
            .ok_or("payload manager call has no exception owner")?;
        let chain = pe.chain(fragment)?;
        if chain.last().is_none_or(|f| f.begin != 0x63d1f0) {
            return Err("payload complete owner differs".into());
        }
        pe.witness(
            0x63d1f0,
            &[
                0x48, 0x8b, 0xc4, 0x55, 0x57, 0x48, 0x8d, 0x68, 0xa1, 0x48, 0x81, 0xec, 0xa8, 0, 0,
                0,
            ],
            false,
        )?;
        pe.witness(
            0x63d4e0,
            &[
                0x66, 0xc7, 0x87, 0xa4, 2, 0, 0, 1, 0, 0xb0, 1, 0x48, 0x81, 0xc4, 0xa8, 0, 0, 0,
                0x5f, 0x5d, 0xc3,
            ],
            false,
        )?;
        let complete = pe.span(0x63d1f0, 0x305, true)?;
        Ok(json!({"schema":"todo880-worker-payload-slice-v1",
            "core_sha256":format!("{:x}",Sha256::digest(core)),
            "witnesses":witnesses,"computed_references":branches,"relocated_table_slots":pointers,
            "complete_owner":{"entry_rva":0x63d1f0,"normal_ret_rva":0x63d4f4,
                "span_bytes":complete.len(),"span_sha256":format!("{:x}",Sha256::digest(complete)),
                "scope":"whole copied interval and static unwind ownership, not exhaustive executed control flow",
                "manager_fragment_chain":chain.iter().map(|f|json!({"begin":f.begin,"end":f.end,"unwind":f.unwind})).collect::<Vec<_>>()},
            "payload_fields":[{"offset":0,"width":8,"value":"table RVA A10EE8"},
                {"offset":8,"width":8,"value":"constructor input RDX; execution/shutdown leaves load it into RCX"},
                {"offset":16,"width":8,"value":"zero in596D50; input RDX in596DA0; NOT ticket storage"},
                {"offset":24,"width":4,"value":"initial1;596DD0 compares1 or decrements/stores; ownership meaning unestablished"}],
            "origin_candidate":{"getter_rva":0x5901a0,"context_table_slot":0x90,
                "receiver_reads":[8,96],"allocation_bytes":32,"allocation_call_rva":0x5901c4,
                "constructor_tail_rva":0x5901d9,"constructor_destination":"RCX from allocation RAX",
                "stored_receiver":"RDX from getter RBX; no later source pointer read inferred",
                "ticket_placement":"only a bridge-owned birth reservation once origin epoch and publication handoff are proven; no native field write"},
            "counter_scope":"local compare/decrement/store bytes only; not an atomic refcounter, ownership transfer, Cancel or join proof",
            "not_established_by_this_audit":["executed Context type and vft identity","payload receiver lifetime",
                "host shared vft+40/+38 ownership semantics","queue publication and992210.RDX identity",
                "992280 publication of this payload","Cancel and reissue completion",
                "safe old-epoch ticket transfer","all-domain Close/Rebuild authority"]}))
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn payload_relative_references_reject_opcode_truncation_and_signed_bounds() {
            assert_eq!(
                rel_target(0x100, &[0xe9, 0xfb, 0xff, 0xff, 0xff], 0xe9, 5, 1).unwrap(),
                0x100
            );
            assert!(rel_target(0, &[0xe9, 0xf0, 0xff, 0xff, 0xff], 0xe9, 5, 1).is_err());
            assert!(rel_target(u32::MAX, &[0xe8, 0, 0, 0, 0], 0xe8, 5, 1).is_err());
            assert!(rel_target(0x100, &[0xe9, 0, 0, 0], 0xe9, 5, 1).is_err());
            assert!(rel_target(0x100, &[0xe8, 0, 0, 0, 0], 0xe9, 5, 1).is_err());
        }
        #[test]
        fn payload_offset10_is_not_a_free_ticket_field() {
            let mut b = vec![0u8; 0x596da0 - 0xc00 + OTHER_CTOR.len()];
            let start = 0x596da0 - 0xc00;
            b[start..].copy_from_slice(OTHER_CTOR);
            assert!(bytes(&b, 0x596da0, OTHER_CTOR).is_ok());
            assert_eq!(&OTHER_CTOR[21..25], &[0x48, 0x89, 0x51, 0x10]);
            b[start + 24] = 0x20;
            assert!(bytes(&b, 0x596da0, OTHER_CTOR).is_err());
        }
        #[test]
        fn payload_wrong_hash_is_rejected_before_section_or_type_interpretation() {
            assert!(audit_worker_payload(b"MZ", b"PE")
                .unwrap_err()
                .contains("pinned"));
        }
    }
}

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

fn rip_reference(
    image: &[u8],
    rva: u32,
    prefix: &[u8],
    suffix: &[u8],
    target: u32,
) -> Result<(), String> {
    let length = prefix.len() + 4 + suffix.len();
    let displacement = i32::try_from(i64::from(target) - i64::from(rva) - length as i64)
        .map_err(|_| "RIP displacement out of range")?;
    let mut expected = prefix.to_vec();
    expected.extend(displacement.to_le_bytes());
    expected.extend_from_slice(suffix);
    bytes(image, rva, &expected)
}

fn byte_b61e00_consumer(core: &[u8], leaf_target: i64) -> Result<Value, String> {
    const TARGET: u32 = 0xb61e00;
    if leaf_target != i64::from(TARGET) {
        return Err("consumer byte differs from leaf reference".into());
    }
    for (rva, prefix, suffix) in [
        (0x7bbf0, &[0x0f, 0xb6, 0x05][..], &[][..]),
        (0x7bbf7, &[0x88, 0x0d][..], &[][..]),
        (0x7c0e9, &[0xc6, 0x05][..], &[1][..]),
        (0x7c687, &[0x80, 0x3d][..], &[0][..]),
        (0x7c7ff, &[0xc6, 0x05][..], &[0][..]),
    ] {
        rip_reference(core, rva, prefix, suffix, TARGET)?;
    }
    // Complete local compare/copies/conditional branch, not just a CMP anchor.
    bytes(
        core,
        0x7c687,
        &[
            0x80, 0x3d, 0x72, 0x57, 0xae, 0x00, 0x00, 0x4c, 0x8b, 0xf2, 0x8b, 0xf1, 0x0f, 0x84,
            0x3d, 0x01, 0x00, 0x00,
        ],
    )?;
    // Local callback-like loop: no flag read in this exact interval. This does
    // not establish reachability from the entry or behavior of an invoked callee.
    bytes(
        core,
        0x7c798,
        &[
            0x48, 0x8b, 0x5f, 0x08, 0x48, 0x3b, 0x5f, 0x10, 0x74, 0x24, 0x48, 0x8b, 0x0b, 0x48,
            0x85, 0xc9, 0x75, 0x06, 0x48, 0x39, 0x4b, 0x08, 0x74, 0x0c, 0x4d, 0x8b, 0xce, 0x4c,
            0x8d, 0x45, 0xe7, 0x8b, 0xd6, 0xff, 0x53, 0x08, 0x48, 0x83, 0xc3, 0x20, 0x48, 0x3b,
            0x5f, 0x10, 0x75, 0xdc,
        ],
    )?;
    bytes(
        core,
        0x7c7d6,
        &[
            0x4c, 0x8d, 0x9c, 0x24, 0x90, 0x00, 0x00, 0x00, 0x49, 0x8b, 0x73, 0x20, 0x4d, 0x8b,
            0x73, 0x28, 0x49, 0x8b, 0xe3, 0x5d, 0xc3,
        ],
    )?;
    branch(core, 0x5fa8d6, 0xe8, 0x7c670)?;
    // The normal-return continuation loads a different indirect receiver and
    // tail-jumps through its table+A0. Role and runtime target remain unknown.
    bytes(
        core,
        0x5fa8db,
        &[
            0x48, 0x8b, 0x0d, 0x9e, 0x01, 0x59, 0x00, 0x4c, 0x8b, 0xc3, 0x8b, 0xd7, 0x48, 0x8b,
            0x01, 0x48, 0x8b, 0x5c, 0x24, 0x30, 0x48, 0x83, 0xc4, 0x20, 0x5f, 0x48, 0xff, 0xa0,
            0xa0, 0x00, 0x00, 0x00,
        ],
    )?;
    Ok(json!({
        "byte_rva":TARGET,
        "rip_instruction_sites":[0x7bbf0,0x7bbf7,0x7c0e9,0x7c687,0x7c7ff],
        "read_site_scan_basis":"LLVM linear text disassembly displayed five explicit RIP-relative references; not an exhaustive indirect/data-flow reference analysis",
        "reader_entry_candidate_rva":0x7c670,"compare_rva":0x7c687,"zero_branch_rva":0x7c693,"zero_branch_target":0x7c7d6,
        "local_loop":{"start_rva":0x7c798,"bytes":46,"indirect_call_rva":0x7c7b9,"call_operand":"[RBX+8]","record_stride":0x20,"back_branch_target":0x7c7a2,"flag_recheck":"absent in this checked interval only"},
        "zero_branch_epilogue":{"rva":0x7c7d6,"bytes":21,"ret_rva":0x7c7ea},
        "caller_5fa8d6_continuation":{"direct_call_rva":0x5fa8d6,"direct_target":0x7c670,"start_rva":0x5fa8db,"bytes":32,"indirect_tail_rva":0x5fa8f4,"table_slot":0xa0,"role_and_target":"not established by this audit","scope":"normal return from the checked direct call falls through this continuation; wrapper entry reachability and actual native return not certified"},
        "unestablished":["entry-to-loop reachability and receiver/register identity","callee role and callback return","all readers and writers via indirect aliases","zero store sealing every AI publisher","already-entered frames and queued work drained","native lifetime order and Close/Rebuild authority"],
        "scope":"zero branch at7C693 targets7C7D6 beyond the checked loop; entry-to-guard and nonzero-path reachability are not established; a byte store alone supplies no join/drain certificate"
    }))
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

// Complete small functions or explicitly bounded local blocks, independently
// decoded with LLVM from the hash-pinned Core. No runtime receiver is inferred
// from sharing an offset, global address, or normal-return call site.
const MANAGER_FLOW_BLOCKS: &[(u32, &[u8])] = &[
    (
        0x35fa5,
        &[
            0xe8, 0xa6, 0xb5, 0x91, 0x00, 0x48, 0x8b, 0xc8, 0xe8, 0x2e, 0x83, 0x60, 0x00, 0x80,
            0xbb, 0x88, 0x02, 0x00, 0x00, 0x00, 0x74, 0x12, 0xe8, 0x90, 0xb5, 0x91, 0x00, 0x48,
            0x8b, 0xc8, 0x48, 0x83, 0xc4, 0x20, 0x5b, 0xe9, 0x13, 0x83, 0x60, 0x00, 0x48, 0x83,
            0xc4, 0x20, 0x5b, 0xc3,
        ],
    ),
    (
        0x63e2e0,
        &[
            0x48, 0x83, 0xec, 0x28, 0xe8, 0xd7, 0xcc, 0xa0, 0xff, 0xe8, 0x72, 0x8b, 0xb4, 0xff,
            0x48, 0x83, 0xc4, 0x28, 0xe9, 0xe9, 0xd8, 0xa3, 0xff,
        ],
    ),
    (0x49b1ba, &[0x44, 0x89, 0xbf, 0xa8, 0x38, 0x00, 0x00]),
    (
        0x49c7e0,
        &[0xc7, 0x81, 0xa8, 0x38, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00],
    ),
    (
        0x49defb,
        &[0xc7, 0x86, 0xa8, 0x38, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00],
    ),
    (
        0x4a1161,
        &[0xc7, 0x81, 0xa8, 0x38, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00],
    ),
    (0x4a16ce, &[0x89, 0xb3, 0xa8, 0x38, 0x00, 0x00]),
    (0x951550, &[0x48, 0x8b, 0x05, 0x11, 0x95, 0x23, 0x00, 0xc3]),
    (0x49c630, &[0x48, 0x8b, 0x81, 0x50, 0x02, 0x00, 0x00, 0xc3]),
    (
        0x49c640,
        &[
            0x48, 0x83, 0xec, 0x28, 0xe8, 0x07, 0x4f, 0x4b, 0x00, 0x48, 0x8b, 0x80, 0x50, 0x02,
            0x00, 0x00, 0x48, 0x83, 0xc4, 0x28, 0xc3,
        ],
    ),
    (
        0x4a7a27,
        &[
            0x48, 0x8b, 0x05, 0x3a, 0x30, 0x6e, 0x00, 0x48, 0x8d, 0x96, 0xd0, 0x00, 0x00, 0x00,
            0x48, 0x89, 0x5c, 0x24, 0x38, 0x4c, 0x8d, 0x05, 0xdf, 0xee, 0x55, 0x00, 0x48, 0x8b,
            0x98, 0x50, 0x02, 0x00, 0x00, 0x48, 0x8b, 0xcb, 0xe8, 0xb0, 0x96, 0xff, 0xff, 0x48,
            0x8b, 0xcb, 0xe8, 0x58, 0x96, 0xff, 0xff,
        ],
    ),
    (
        0x4a1100,
        &[
            0x48, 0x83, 0xec, 0x38, 0x4d, 0x85, 0xc0, 0xc7, 0x44, 0x24, 0x28, 0x00, 0x00, 0x00,
            0x00, 0xc7, 0x44, 0x24, 0x2c, 0x1f, 0x00, 0x00, 0x00, 0x48, 0x8d, 0x05, 0xa2, 0x4e,
            0x52, 0x00, 0x49, 0x0f, 0x45, 0xc0, 0x4c, 0x8d, 0x4c, 0x24, 0x20, 0x48, 0x89, 0x44,
            0x24, 0x20, 0x4c, 0x8d, 0x81, 0x00, 0x04, 0x00, 0x00, 0x0f, 0x28, 0x44, 0x24, 0x20,
            0x48, 0x8b, 0x89, 0x10, 0x04, 0x00, 0x00, 0x66, 0x0f, 0x7f, 0x44, 0x24, 0x20, 0xe8,
            0xc6, 0x10, 0x4f, 0x00, 0x48, 0x83, 0xc4, 0x38, 0xc3,
        ],
    ),
    (
        0x4a10b0,
        &[
            0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x80, 0xb9, 0xac, 0x38, 0x00, 0x00, 0x00, 0x48,
            0x8b, 0xd9, 0x75, 0x2b, 0x48, 0x8b, 0x0d, 0xb7, 0x99, 0x6e, 0x00, 0x48, 0x8b, 0x01,
            0xff, 0x50, 0x30, 0x88, 0x83, 0xad, 0x38, 0x00, 0x00, 0x84, 0xc0, 0x74, 0x0d, 0x48,
            0x8b, 0x0d, 0xa0, 0x99, 0x6e, 0x00, 0x48, 0x8b, 0x01, 0xff, 0x50, 0x38, 0xc6, 0x83,
            0xac, 0x38, 0x00, 0x00, 0x01, 0x48, 0x8b, 0x8b, 0x10, 0x04, 0x00, 0x00, 0x48, 0x83,
            0xc4, 0x20, 0x5b, 0xe9, 0x82, 0x11, 0x4f, 0x00,
        ],
    ),
];

// Call, returned RAX comparison, and complete JNE, with no unverified gap.
const MANAGER_STATE_PROBES: &[(u32, u32, u32, &[u8])] = &[
    (
        0x2f929c,
        0x49c640,
        0x2f933a,
        &[
            0xe8, 0x9f, 0x33, 0x1a, 0x00, 0x83, 0xb8, 0xa8, 0x38, 0x00, 0x00, 0x02, 0x0f, 0x85,
            0x8c, 0x00, 0x00, 0x00,
        ],
    ),
    (
        0x2f9366,
        0x49c640,
        0x2f9464,
        &[
            0xe8, 0xd5, 0x32, 0x1a, 0x00, 0x83, 0xb8, 0xa8, 0x38, 0x00, 0x00, 0x02, 0x0f, 0x85,
            0xec, 0x00, 0x00, 0x00,
        ],
    ),
    (
        0x5242e2,
        0x49c640,
        0x52433b,
        &[
            0xe8, 0x59, 0x83, 0xf7, 0xff, 0x83, 0xb8, 0xa8, 0x38, 0x00, 0x00, 0x02, 0x75, 0x4b,
        ],
    ),
    (
        0x52484b,
        0x49c640,
        0x524890,
        &[
            0xe8, 0xf0, 0x7d, 0xf7, 0xff, 0x83, 0xb8, 0xa8, 0x38, 0x00, 0x00, 0x02, 0x75, 0x37,
        ],
    ),
    (
        0x52493b,
        0x49c640,
        0x524982,
        &[
            0xe8, 0x00, 0x7d, 0xf7, 0xff, 0x83, 0xb8, 0xa8, 0x38, 0x00, 0x00, 0x02, 0x75, 0x39,
        ],
    ),
    (
        0x524cfd,
        0x49c640,
        0x524ddd,
        &[
            0xe8, 0x3e, 0x79, 0xf7, 0xff, 0x83, 0xb8, 0xa8, 0x38, 0x00, 0x00, 0x02, 0x0f, 0x85,
            0xce, 0x00, 0x00, 0x00,
        ],
    ),
    (
        0x524ef2,
        0x49c640,
        0x524f3e,
        &[
            0xe8, 0x49, 0x77, 0xf7, 0xff, 0x83, 0xb8, 0xa8, 0x38, 0x00, 0x00, 0x02, 0x75, 0x3e,
        ],
    ),
    (
        0x52526e,
        0x49c640,
        0x5252b3,
        &[
            0xe8, 0xcd, 0x73, 0xf7, 0xff, 0x83, 0xb8, 0xa8, 0x38, 0x00, 0x00, 0x02, 0x75, 0x37,
        ],
    ),
    (
        0x5252ed,
        0x49c640,
        0x525332,
        &[
            0xe8, 0x4e, 0x73, 0xf7, 0xff, 0x83, 0xb8, 0xa8, 0x38, 0x00, 0x00, 0x02, 0x75, 0x37,
        ],
    ),
    (
        0x561286,
        0x49c630,
        0x5614fb,
        &[
            0xe8, 0xa5, 0xb3, 0xf3, 0xff, 0x83, 0xb8, 0xa8, 0x38, 0x00, 0x00, 0x02, 0x0f, 0x85,
            0x63, 0x02, 0x00, 0x00,
        ],
    ),
];

fn manager_receiver_flow(core: &[u8]) -> Result<Value, String> {
    for &(rva, block) in MANAGER_FLOW_BLOCKS {
        bytes(core, rva, block)?;
    }
    for (rva, op, target) in [
        (0x35fa5, 0xe8, 0x951550),
        (0x35fad, 0xe8, 0x63e2e0),
        (0x35fbb, 0xe8, 0x951550),
        (0x35fc8, 0xe9, 0x63e2e0),
        (0x63e2e4, 0xe8, 0x4afc0),
        (0x63e2e9, 0xe8, 0x186e60),
        (0x63e2f2, 0xe9, 0x7bbe0),
        (0x49c644, 0xe8, 0x951550),
        (0x4a7a4b, 0xe8, 0x4a1100),
        (0x4a7a53, 0xe8, 0x4a10b0),
        (0x4a1145, 0xe8, 0x992210),
        (0x4a10f9, 0xe9, 0x992280),
    ] {
        branch(core, rva, op, target)?;
    }
    for rva in [0x951550, 0x4a7a27] {
        rip_reference(core, rva, &[0x48, 0x8b, 0x05], &[], 0xb8aa68)?;
    }
    let mut probes = Vec::new();
    for &(call, getter, target, block) in MANAGER_STATE_PROBES {
        bytes(core, call, block)?;
        branch(core, call, 0xe8, getter)?;
        let displacement = match block.get(12..) {
            Some([0x75, rel]) => i64::from(*rel as i8),
            Some([0x0f, 0x85, a, b, c, d]) => i64::from(i32::from_le_bytes([*a, *b, *c, *d])),
            _ => return Err("state comparison lacks complete JNE".into()),
        };
        if i64::from(call) + block.len() as i64 + displacement != i64::from(target) {
            return Err("state comparison JNE target mismatch".into());
        }
        probes.push(json!({"call_rva":call,"getter_rva":getter,"compare_rva":call+5,"not_equal_branch_target":target,"receiver_basis":if getter==0x49c640 {"returned value loaded from [global B8AA68]+250 at this invocation"} else {"returned value loaded from incoming RCX+250; not assumed equal to global receiver"}}));
    }
    Ok(json!({
        "global_pointer_rva":0xb8aa68,
        "getter":{"leaf_rva":0x951550,"manager_rva":0x49c640,"explicit_receiver_rva":0x49c630,"field_offset":0x250},
        "state2_probes":probes,
        "local_submit_spawn":{"start_rva":0x4a7a27,"bytes":49,"manager_load_rva":0x4a7a41,"push_call_rva":0x4a7a4b,"spawn_call_rva":0x4a7a53,"receiver_continuity":"RBX loaded from [global B8AA68]+250 and passed as RCX to both calls; conditional on normal Push return preserving Win64 RBX"},
        "push_wrapper":{"entry_rva":0x4a1100,"bytes":79,"set_address_offset":0x400,"list_id_offset":0x410,"state38a8_read":"absent in this complete checked wrapper; invoked host callee semantics not certified"},
        "spawn_wrapper":{"entry_rva":0x4a10b0,"bytes":78,"byte_guard_offset":0x38ac,"callback_result_offset":0x38ad,"indirect_slots":[0x30,0x38],"indirect_receiver_global_rva":0xb8aa80,"state38a8_read":"absent in this complete checked wrapper; indirect callee semantics not certified"},
        "higher_dispatch_continuation":{"wrapper_rva":0x63e2e0,"wrapper_bytes":23,"outer_call_rva":0x63e2e4,"after_outer_call_rva":0x63e2e9,"after_outer_target":0x186e60,"tail_target":0x7bbe0,"caller_local_rva":0x35fa5,"caller_local_bytes":46,"caller_call_rva":0x35fad,"after_wrapper_compare_rva":0x35fb2,"compare_operand":"byte [RBX+288]","zero_branch_target":0x35fcd,"nonzero_path":"calls951550 again, copies returned global pointer toRCX, restores caller stack/RBX, tail-jumps63E2E0 at35FC8","scope":"normal returns with Win64 RBX preservation; caller entry reachability, RBX provenance/type/lifetime and callee semantics unestablished;4AFC0 exit alone does not establish this caller exit"},
        "state_store_sites":[{"rva":0x49b1ba,"base":"RDI","source":"R15D; value and entry receiver continuity not established here"},{"rva":0x49c7e0,"base":"RCX","immediate":1},{"rva":0x49defb,"base":"RSI","immediate":2,"continuity":"not connected to49C7E0 across the unverified intervening function body"},{"rva":0x4a1161,"base":"RCX","immediate":3},{"rva":0x4a16ce,"base":"RBX","source":"ESI; value and path continuity not established here"}],
        "scope":"nine global-derived comparisons and one explicit-receiver comparison, plus five store sites: fifteen displayed explicit offset38A8 sites only; not exhaustive alias analysis",
        "unestablished":["global pointer and manager field stability across separate calls or worlds","entry reachability of bounded local blocks","state2 checks sealing every publisher or already-passed caller","pending list publication and same-set full join","synchronous4AFC0/34840 and their higher callers, including35F70, exited without reentry","Close/Rebuild authority and native receiver lifetime"]
    }))
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
        "schema":"todo880-native-job-lifecycle-static-v6",
        "core_sha256":core_sha,"host_sha256":host_sha,"witnesses":witnesses,
        "manager":{"from_world_offset":0x250,"job_set_offset":0x400,"list_id_offset":0x410,"lifecycle_state_offset":0x38a8},
        "abi":{
            "push":{"rcx":"list ID (native host uses low32, IDs1..64)","rdx":"native job object", "r8":"manager+400 job-set address", "r9":"pointer to 16-byte metadata copy"},
            "wait":{"rcx":"same job-set address", "rdx":"pointer to 16-byte metadata copy", "result":"normal return, not RAX=true; host clears set+8 after native arena wait"}
        },
        "receipt_candidates":{"wait_return_core":0x4a1196,"conditional_host_callback_merge_core":0x4a11b3,"free_list_return_core":0x4a120e,"host_wait_pointer_clear":0x985e50},
        "file_offset_basis":"SHA-pinned images with independently verified text RVA1000/raw400; Core rdata RVA9B7000/raw9B5C00 virtual size1A8124 and preferred ImageBase180000000; not a general PE parser",
        "release_leaf":{"wrapper_rva":0x58ff70,"leaf_rva":0x7bbf0,"instruction_bytes":14,"same_byte_target_rva":read_target,"behavior":"non-atomic read then write of one RIP-relative byte; not world teardown","world_shutdown_link_evidence":"not established by this audit"},
        "byte_b61e00_consumer":byte_b61e00_consumer(core, read_target)?,
        "manager_receiver_flow":manager_receiver_flow(core)?,
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

    #[test]
    fn rip_reference_accounts_for_trailing_immediate_and_negative_displacement() {
        let mut image = vec![0; 0x108];
        image[0x100..0x107].copy_from_slice(&[0xc6, 0x05, 0xe9, 0xff, 0xff, 0xff, 1]);
        assert!(rip_reference(&image, 0xd00, &[0xc6, 0x05], &[1], 0xcf0).is_ok());
        assert!(rip_reference(&image, 0xd00, &[0xc6, 0x05], &[0], 0xcf0).is_err());
        assert!(rip_reference(&image, 0xd00, &[0xc6, 0x05], &[1], 0xcf1).is_err());
        assert!(rip_reference(&image[..0x106], 0xd00, &[0xc6, 0x05], &[1], 0xcf0).is_err());
        assert!(rip_reference(&image, 0xbff, &[0xc6, 0x05], &[1], 0xcf0).is_err());
        assert!(rip_reference(&image, 0xd00, &[0xc6, 0x05], &[1], u32::MAX).is_err());
        assert!(byte_b61e00_consumer(&[], 0xb61e01)
            .unwrap_err()
            .contains("differs from leaf"));
    }

    #[test]
    fn manager_flow_rejects_changed_receiver_and_branch_and_keeps_explicit_source_separate() {
        let mut image = vec![0; 0x951558 - 0xc00];
        for &(rva, block) in MANAGER_FLOW_BLOCKS {
            let offset = (rva - 0xc00) as usize;
            image[offset..offset + block.len()].copy_from_slice(block);
        }
        for &(rva, _, _, block) in MANAGER_STATE_PROBES {
            let offset = (rva - 0xc00) as usize;
            image[offset..offset + block.len()].copy_from_slice(block);
        }
        let flow = manager_receiver_flow(&image).unwrap();
        assert_eq!(flow["state2_probes"].as_array().unwrap().len(), 10);
        assert_eq!(flow["state_store_sites"].as_array().unwrap().len(), 5);
        assert_eq!(flow["state2_probes"][9]["getter_rva"], 0x49c630);
        assert!(flow["state2_probes"][9]["receiver_basis"]
            .as_str()
            .unwrap()
            .contains("not assumed equal"));
        // Change the intervening MOV RCX,RBX to MOV RCX,RDX. The former
        // direct-call anchors alone would still match, but identity no longer does.
        assert_eq!(
            flow["higher_dispatch_continuation"]["caller_local_bytes"],
            46
        );
        for rva in [
            0x4a7a52, 0x49c64c, 0x951553, 0x5252ed, 0x5242ef, 0x35fb9, 0x63e2e9,
        ] {
            let offset = (rva - 0xc00) as usize;
            image[offset] ^= 1;
            assert!(manager_receiver_flow(&image).is_err(), "mutation {rva:#x}");
            image[offset] ^= 1;
        }
        assert!(manager_receiver_flow(&image[..image.len() - 1]).is_err());
    }
}
