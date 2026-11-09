//! Fixed-file evidence for the real JobManager path; no handles or invocation.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

// Explicit audit revision, independent of runtime admission pins. Both complete
// file buffers must match this pair before any fixed RVA is interpreted.
const IMAGE_REVISION: &str = "steam-26407122";
const CORE_SHA: &str = "092abfbb2176f8df04b5212f819dc269dd83aea866ff05aaf489904a1ff7c41f";
const HOST_SHA: &str = "d1a0cfd439e7451be515f47595cfa0b6b983f56d1faf4ea434a4ce3b21491ebe";

fn require_image_pair(core_sha: &str, host_sha: &str) -> Result<(), String> {
    if core_sha != CORE_SHA || host_sha != HOST_SHA {
        return Err(
            "not both pinned primary files for steam-26407122; no ABI interpretation".into(),
        );
    }
    Ok(())
}

/// Complete synchronous caller evidence, without enabling a runtime adapter.
/// The selected revision's base audit is also returned without enabling hooks.
pub fn audit_outer_frames(core: &[u8], host: &[u8]) -> Result<Value, String> {
    let base = audit(core, host)?; // both pinned buffers before ABI interpretation
    Ok(
        json!({"schema":"native-outer-frames-static-v1", "base_audit":base,
        "outer_frame_slice":worker_payload::inspect_outer_frames(core)?,
        "certificate_issued":false,"product_acceptance":false}),
    )
}

/// Opt-in slice for the selected image revision; no runtime activation.
/// Hashes refer to these borrowed file buffers, never a later path reread.
pub fn audit_worker_payload(core: &[u8], host: &[u8]) -> Result<Value, String> {
    let base = audit(core, host)?; // includes BOTH hash gates before interpreting ABI
    let payload = worker_payload::inspect(core)?;
    Ok(json!({"schema":"todo880-native-worker-payload-static-v2",
        "base_audit":base,"payload_slice":payload,
        "pending_publication_slice":worker_payload::inspect_pending(host)?,
        "structural_declarations":{"process_open_calls":0,"dll_load_calls":0,
            "runtime_feature_activation_calls":0,
            "basis":"file-buffer-only code structure; not measured process or runtime counters"}}))
}

/// Local queue-consumer and allocation-return evidence, never runtime authority.
/// Existing selectors and their output remain unchanged.
pub fn audit_queue_consumer(core: &[u8], host: &[u8]) -> Result<Value, String> {
    let base = audit_worker_payload(core, host)?;
    Ok(
        json!({"schema":"native-queue-consumer-static-v1", "base_worker_audit":base,
        "queue_consumer_slice":worker_payload::inspect_consumer(host)?,
        "certificate_issued":false,"product_acceptance":false}),
    )
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
            Self::parse_image(image, true)
        }
        fn parse_executable(image: &'a [u8]) -> Result<Self, String> {
            Self::parse_image(image, false)
        }
        fn parse_image(image: &'a [u8], require_relocations: bool) -> Result<Self, String> {
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
            if !require_relocations && u16_at(image, coff + 18)? & 0x2002 != 2 {
                return Err("pending code audit requires an executable, not a DLL".into());
            }
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
            if pdata_size == 0 || !pdata_size.is_multiple_of(12) || pdata_size / 12 > 131072 {
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
            if (reloc_rva == 0) != (reloc_size == 0) {
                return Err("payload relocation directory address/size disagree".into());
            }
            if reloc_size == 0 {
                if require_relocations {
                    return Err("payload relocation directory absent".into());
                }
                return Ok(pe);
            }
            let reloc = pe.span(reloc_rva, reloc_size, false)?;
            let mut pos = 0usize;
            while pos < reloc.len() {
                let page = u32_at(reloc, pos)?;
                let size = u32_at(reloc, pos + 4)? as usize;
                if size < 8 || !size.is_multiple_of(2) {
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

    /// Only the explicit --worker-payload result gains this additional slice.
    pub(super) fn inspect_pending(host: &[u8]) -> Result<Value, String> {
        pending_publication::inspect(&Pe::parse_executable(host)?)
    }

    pub(super) fn inspect_consumer(host: &[u8]) -> Result<Value, String> {
        pending_publication::inspect_consumer(&Pe::parse_executable(host)?)
    }

    pub(super) fn inspect_outer_frames(core: &[u8]) -> Result<Value, String> {
        outer_frames::inspect(&Pe::parse(core)?)
    }

    mod outer_frames {
        use super::*;

        struct Frame {
            begin: u32,
            end: u32,
            sha: &'static str,
            fragments: &'static [(u32, u32, u32, &'static str)],
        }

        // Each partition includes the epilogues, not just the first pdata row.
        const FRAMES: &[Frame] = &[
            Frame {
                begin: 0x35f70,
                end: 0x35fd3,
                sha: "22cf132e7f44201dec040a64f8450ff52f6ce7362230e717226ed0b9a5bc60b6",
                fragments: &[(0x35f70, 0x35fd3, 0xb45024, "0106020006320230")],
            },
            Frame {
                begin: 0x36180,
                end: 0x363ca,
                sha: "6371ece2fe74720ed1efb9b9a9550abc872d5fe120dda9e970f6139e4cc884a3",
                fragments: &[
                    (
                        0x36180,
                        0x36271,
                        0xac3884,
                        "010b05000b0152000470035002300000",
                    ),
                    (
                        0x36271,
                        0x3632d,
                        0xac3894,
                        "210802000864580080610300716203008438ac00",
                    ),
                    (
                        0x3632d,
                        0x363ca,
                        0xac38a8,
                        "2100000080610300716203008438ac00",
                    ),
                ],
            },
            Frame {
                begin: 0x39920,
                end: 0x39dee,
                sha: "7284aae0f8821587b27cff4f6f7cb0926373eaf6ed158156ab78b1deeb569593",
                fragments: &[
                    (0x39920, 0x39942, 0xac3bfc, "011304001301550004e00250"),
                    (
                        0x39942,
                        0x3995b,
                        0xac3c08,
                        "2108020008345a002099030042990300fc3bac00",
                    ),
                    (
                        0x3995b,
                        0x39ddb,
                        0xac3c1c,
                        "2120080020f4520018c453001074540008645b00429903005b990300083cac00",
                    ),
                    (
                        0x39ddb,
                        0x39de3,
                        0xac3c3c,
                        "21000000429903005b990300083cac00",
                    ),
                    (
                        0x39de3,
                        0x39dee,
                        0xac3c4c,
                        "210000002099030042990300fc3bac00",
                    ),
                ],
            },
            Frame {
                begin: 0x4182b0,
                end: 0x418860,
                sha: "1facbb5c67cf9290a51cb8be4fe67b1f7c73e906e57cc6b9d0c8165b66396991",
                fragments: &[
                    (0x4182b0, 0x4182f3, 0xaff1f8, "010a04000a011b0003700230"),
                    (
                        0x4182f3,
                        0x41841a,
                        0xaff204,
                        "2108020008d41800b0824100f3824100f8f1af00",
                    ),
                    (
                        0x41841a,
                        0x418427,
                        0xaff218,
                        "2100020000d41800b0824100f3824100f8f1af00",
                    ),
                    (
                        0x418427,
                        0x41884e,
                        0xaff22c,
                        "21280a0028f4160020e4170018c4190010641a0008541f001a8441002784410018f2af00",
                    ),
                    (
                        0x41884e,
                        0x418856,
                        0xaff250,
                        "210000001a8441002784410018f2af00",
                    ),
                    (
                        0x418856,
                        0x418860,
                        0xaff260,
                        "21000000b0824100f3824100f8f1af00",
                    ),
                ],
            },
            Frame {
                begin: 0x4afc0,
                end: 0x4b87b,
                sha: "0b50e7656ea4ce7be5eb3fbe950b25116fa14f7514932fc360a3fbd39f636867",
                fragments: &[(
                    0x4afc0,
                    0x4b87b,
                    0xac4ddc,
                    "011f0b001f3492001f01880010f00ee00cd00ac00870076006500000",
                )],
            },
        ];

        // All instructions are position-independent push, sub rsp or mov/lea
        // using rsp. Lengths cover E9 + three NOPs, not merely its first 5 bytes.
        type Prefix = (
            u32,
            &'static [u8],
            &'static [usize],
            &'static [&'static str],
        );
        const PREFIXES: &[Prefix] = &[
            (
                0x35f70,
                &[0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0xd9],
                &[2, 4, 3],
                &["RCX"],
            ),
            (
                0x36180,
                &[0x40, 0x53, 0x55, 0x57, 0x48, 0x81, 0xec, 0x90, 2, 0, 0],
                &[2, 1, 1, 7],
                &["RCX"],
            ),
            (
                0x39920,
                &[
                    0x40, 0x55, 0x41, 0x56, 0x48, 0x8d, 0xac, 0x24, 0x58, 0xfe, 0xff, 0xff,
                ],
                &[2, 2, 8],
                &["RCX"],
            ),
            (
                0x4182b0,
                &[0x40, 0x53, 0x57, 0x48, 0x81, 0xec, 0xd8, 0, 0, 0],
                &[2, 1, 7],
                &["RCX", "EDX"],
            ),
        ];
        fn hex(b: &[u8]) -> String {
            b.iter().map(|x| format!("{x:02x}")).collect()
        }
        fn frame(pe: &Pe<'_>, spec: &Frame) -> Result<Value, String> {
            let length = spec
                .end
                .checked_sub(spec.begin)
                .filter(|n| *n > 0)
                .ok_or("outer empty/reversed frame")?;
            let mut next = spec.begin;
            let mut fragments = Vec::new();
            for &(begin, end, unwind, expected) in spec.fragments {
                if begin != next || end <= begin || end > spec.end {
                    return Err("outer incomplete fragment partition".into());
                }
                let f = Function { begin, end, unwind };
                if pe.function(begin) != Some(f) || pe.function(end - 1) != Some(f) {
                    return Err("outer pdata range differs".into());
                }
                let chain = pe.chain(f)?;
                if chain.last().is_none_or(|f| f.begin != spec.begin)
                    || chain.iter().any(|f| {
                        !spec.fragments.iter().any(|&(a, z, u, _)| {
                            *f == (Function {
                                begin: a,
                                end: z,
                                unwind: u,
                            })
                        })
                    })
                {
                    return Err("outer CHAININFO owner differs".into());
                }
                let h = pe.span(unwind, 4, false)?;
                let aligned = 4 + ((usize::from(h[2]) + 1) & !1) * 2;
                let flags = h[0] >> 3;
                let size = aligned
                    + if flags & 4 != 0 {
                        12
                    } else if flags & 3 != 0 {
                        4
                    } else {
                        0
                    };
                let raw = pe.span(unwind, size, false)?;
                if hex(raw) != expected || u32::from(h[1]) > end - begin {
                    return Err("outer unwind bytes differ".into());
                }
                fragments.push(json!({"begin":begin,"end":end,"unwind":unwind,
                    "unwind_bytes_hex":hex(raw),
                    "chain":chain.iter().map(|f|json!({"begin":f.begin,"end":f.end,"unwind":f.unwind})).collect::<Vec<_>>()}));
                next = end;
            }
            if next != spec.end {
                return Err("outer incomplete frame end".into());
            }
            let body = pe.span(spec.begin, length as usize, true)?;
            if format!("{:x}", Sha256::digest(body)) != spec.sha {
                return Err("outer complete frame SHA differs".into());
            }
            Ok(json!({"entry_rva":spec.begin,"end_rva":spec.end,
                "span_sha256":spec.sha,"fragments":fragments,
                "scope":"complete fixed disk interval and unwind ownership; not all incoming edges or execution"}))
        }
        fn prefix(
            pe: &Pe<'_>,
            entry: u32,
            expected: &[u8],
            lengths: &[usize],
        ) -> Result<Value, String> {
            if entry & 7 != 0
                || lengths.is_empty()
                || lengths.contains(&0)
                || lengths.iter().sum::<usize>() != expected.len()
                || expected.len() < 8
            {
                return Err("outer CAS8 prefix alignment/instruction partition".into());
            }
            let actual = pe.span(entry, expected.len(), true)?;
            if actual != expected {
                return Err("outer complete relocated prefix differs".into());
            }
            let mut end = 0;
            let boundaries = lengths
                .iter()
                .map(|len| {
                    end += len;
                    end
                })
                .collect::<Vec<_>>();
            Ok(
                json!({"entry_rva":entry,"cas_word_bytes_hex":hex(&actual[..8]),
                "stolen_bytes_hex":hex(actual),"stolen_length":actual.len(),
                "instruction_end_offsets":boundaries,"relative_relocations":[],
                "patch_scope":"aligned CAS8 E9+NOP3; validate whole stolen span; concurrent instruction safety and trampoline unwind require separate validation"}),
            )
        }
        fn witnesses(pe: &Pe<'_>) -> Result<Vec<Value>, String> {
            let mut out = Vec::new();
            for (r, op, target) in [
                (0x35fad, 0xe8, 0x63e2e0),
                (0x35fc8, 0xe9, 0x63e2e0),
                (0x363a0, 0xe8, 0x63e2e0),
                (0x39da0, 0xe8, 0x63e2e0),
                (0x39db6, 0xe8, 0x63e2e0),
                (0x418599, 0xe8, 0x34840),
                (0x41877e, 0xe8, 0x34840),
                (0x63e2e4, 0xe8, 0x4afc0),
                (0x4b16d, 0xe8, 0x5e2d0),
            ] {
                let target_actual = rel_target(r, pe.span(r, 5, true)?, op, 5, 1)?;
                if target_actual != target {
                    return Err("outer branch target differs".into());
                }
                out.push(
                    json!({"rva":r,"kind":if op==0xe8 {"call"} else {"tail"},"target_rva":target}),
                );
            }
            for (r, b) in [
                (0x363c3, &[0x48, 0xff, 0xa0, 0x18, 1, 0, 0][..]),
                (0x35fd2, &[0xc3][..]),
                (0x39ded, &[0xc3][..]),
                (0x418419, &[0xc3][..]),
                (0x41885f, &[0xc3][..]),
                (0x4b87a, &[0xc3][..]),
                (
                    0x4b172,
                    &[0x8b, 0x8d, 0xb0, 0, 0, 0, 0x48, 0x83, 0xc7, 4][..],
                ),
            ] {
                out.push(pe.witness(r, b, false)?);
            }
            Ok(out)
        }
        pub(super) fn inspect(pe: &Pe<'_>) -> Result<Value, String> {
            let frames = FRAMES
                .iter()
                .map(|f| frame(pe, f))
                .collect::<Result<Vec<_>, _>>()?;
            let prefixes=PREFIXES.iter().map(|&(entry,bytes,lengths,args)| {
                let mut value=prefix(pe,entry,bytes,lengths)?;
                value["consumed_integer_inputs"]=json!(args);
                value["incoming_stack_argument_count"]=json!(0);
                value["abi_scope"]=json!("manual instruction/dataflow inspection of this hashed interval; no source-language signature or callee behavior inferred");
                Ok(value)
            }).collect::<Result<Vec<_>,String>>()?;
            Ok(json!({"schema":"native-synchronous-outer-slice-v1",
                "frames":frames,"gateway_prefixes":prefixes,"call_and_exit_witnesses":witnesses(pe)?,
                "callsite_owners":[{"call_rva":0x363a0,"entry_rva":0x36180},
                    {"call_rva":0x39da0,"entry_rva":0x39920},{"call_rva":0x39db6,"entry_rva":0x39920},
                    {"call_rva":0x418599,"entry_rva":0x4182b0},{"call_rva":0x41877e,"entry_rva":0x4182b0}],
                "stack_basis":[{"entry_rva":0x36180,"body_rsp_below_entry":0x2a8,"home_local_offsets":[0x2b0,0x2b8,0x2c0]},
                    {"entry_rva":0x39920,"body_rsp_below_entry":0x2b8,"rbp_below_entry":0x1b8,"home_save_offsets":[0x2d0,0x2d8]},
                    {"entry_rva":0x4182b0,"body_rsp_below_entry":0xe8,"home_local_offsets":[0xf0,0xf8,0x100,0x108]}],
                "return_contract":"retain through original normal return and SEH, including35FC8 direct tail and363C3 indirect tail; preserve GPR/XMM/FLAGS/LastError and native return bits",
                "adapter_contract":["count every native entry independently of telemetry, including after seal; never skip original execution",
                    "nested entry inherits the tracked outer generation; a new outer after seal makes closure sticky unresolved",
                    "serialize late entry, finalization and rebuild on the same generation; original release SEH is not normal-return finalization",
                    "do not use WorldFlights dispatch as a native dispatch/birth-epoch receipt"],
                "limits":["only four named synchronous entry groups; not all publishers or queued native workers",
                    "4AFC0 calls5E2D0 then continues iteration and subsequent work; inner Wait return is not outer return",
                    "no all-domain natural producer seal, pending publication, cancellation or same-set full join proof",
                    "entry observation cannot attribute a delayed dispatch to its original world without a birth receipt"],
                "certificate_issued":false,"all_domain_proven":false,"product_acceptance":false}))
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            const FIXTURE: Frame = Frame {
                begin: 0x1000,
                end: 0x1010,
                sha: "2f2bc2159a8d8994a7c15c44d2e60bb6db71fed09be7b8f169f02be96baae01a",
                fragments: &[
                    (0x1000, 0x1008, 0x3000, "01000000"),
                    (0x1008, 0x1010, 0x3010, "21000000001000000810000000300000"),
                ],
            };
            fn image() -> Vec<u8> {
                let mut b = vec![0; 0x2000];
                b[..15].fill(0x90);
                b[15] = 0xc3;
                b[0x1000..0x1004].copy_from_slice(&[1, 0, 0, 0]);
                b[0x1010..0x1020]
                    .copy_from_slice(&[0x21, 0, 0, 0, 0, 0x10, 0, 0, 8, 0x10, 0, 0, 0, 0x30, 0, 0]);
                b
            }
            fn pe(b: &[u8]) -> Pe<'_> {
                Pe {
                    image: b,
                    base: 0x180000000,
                    sections: vec![
                        Section {
                            name: *b".text\0\0\0",
                            rva: 0x1000,
                            virtual_size: 0x1000,
                            raw: 0,
                            size: 0x1000,
                            flags: 0x60000020,
                        },
                        Section {
                            name: *b".rdata\0\0",
                            rva: 0x3000,
                            virtual_size: 0x1000,
                            raw: 0x1000,
                            size: 0x1000,
                            flags: 0x40000040,
                        },
                    ],
                    functions: vec![
                        Function {
                            begin: 0x1000,
                            end: 0x1008,
                            unwind: 0x3000,
                        },
                        Function {
                            begin: 0x1008,
                            end: 0x1010,
                            unwind: 0x3010,
                        },
                    ],
                    relocations: Default::default(),
                }
            }
            fn prefix_pe(b: &[u8], entry: u32) -> Pe<'_> {
                Pe {
                    image: b,
                    base: 0x180000000,
                    sections: vec![Section {
                        name: *b".text\0\0\0",
                        rva: entry,
                        virtual_size: b.len() as u32,
                        raw: 0,
                        size: b.len(),
                        flags: 0x60000020,
                    }],
                    functions: vec![],
                    relocations: Default::default(),
                }
            }
            #[test]
            fn outer_complete_frame_rejects_body_unwind_and_parent_mutations() {
                let mut b = image();
                let v = frame(&pe(&b), &FIXTURE).unwrap();
                assert_eq!(v["fragments"].as_array().unwrap().len(), 2);
                for offset in [1, 8, 15, 0x1000, 0x1010, 0x1014, 0x1018, 0x101c] {
                    b[offset] ^= 1;
                    assert!(frame(&pe(&b), &FIXTURE).is_err(), "offset {offset:x}");
                    b[offset] ^= 1;
                }
            }
            #[test]
            fn outer_fragment_entry_missing_epilogue_and_wrong_code_section_reject() {
                let b = image();
                frame(&pe(&b), &FIXTURE).unwrap();
                let mut p = pe(&b);
                p.functions[1].begin += 1;
                assert!(frame(&p, &FIXTURE).is_err());
                let mut p = pe(&b);
                p.functions.pop();
                assert!(frame(&p, &FIXTURE).is_err());
                let missing = Frame {
                    fragments: &FIXTURE.fragments[..1],
                    ..FIXTURE
                };
                assert!(frame(&pe(&b), &missing).is_err());
                let fragment = Frame {
                    begin: 0x1008,
                    fragments: &FIXTURE.fragments[1..],
                    ..FIXTURE
                };
                assert!(frame(&pe(&b), &fragment).is_err());
                let mut p = pe(&b);
                p.sections[0].flags &= !0x20000000;
                assert!(frame(&p, &FIXTURE).is_err());
            }
            #[test]
            fn outer_cas8_prefix_checks_bytes_beyond_atomic_word_and_whole_instructions() {
                for &(entry, bytes, lengths, _) in PREFIXES {
                    let mut b = vec![0; bytes.len()];
                    b.copy_from_slice(bytes);
                    let v = prefix(&prefix_pe(&b, entry), entry, bytes, lengths).unwrap();
                    assert_eq!(v["stolen_length"], bytes.len());
                    assert!(prefix(&prefix_pe(&b, entry), entry, bytes, &[8]).is_err());
                    for i in 0..bytes.len() {
                        b[i] ^= 1;
                        assert!(
                            prefix(&prefix_pe(&b, entry), entry, bytes, lengths).is_err(),
                            "{entry:x}+{i}"
                        );
                        b[i] ^= 1;
                    }
                    assert!(prefix(&prefix_pe(&b[..8], entry), entry, bytes, lengths).is_err());
                    assert!(prefix(&prefix_pe(&b, entry), entry + 1, bytes, lengths).is_err());
                }
                assert_eq!(
                    PREFIXES.iter().map(|p| p.1.len()).collect::<Vec<_>>(),
                    [9, 11, 12, 10]
                );
            }
            #[test]
            fn outer_both_file_hashes_gate_new_interpretation() {
                assert!(audit_outer_frames(b"MZ", b"PE")
                    .unwrap_err()
                    .contains("pinned"));
            }
        }
    }

    mod pending_publication {
        use super::*;
        mod consumer;

        pub(super) fn inspect_consumer(pe: &Pe<'_>) -> Result<Value, String> {
            consumer::inspect(pe)
        }

        struct Frame {
            begin: u32,
            end: u32,
            sha: &'static str,
            fragments: &'static [(u32, u32, u32, &'static str)],
        }
        const FRAMES: &[Frame] = &[
            Frame {
                begin: 0x985f40,
                end: 0x98600a,
                sha: "800bcfe9adbff77b1009377e5a1060c5d04d48244d28e2862236aa8b7627e905",
                fragments: &[(
                    0x985f40,
                    0x98600a,
                    0x11c3150,
                    "0114080014640b0014540a001434090014521070",
                )],
            },
            Frame {
                begin: 0x986010,
                end: 0x98614e,
                sha: "950be5fcf473da61ba436dce055ab6c17f5ba71f8bfaf51c5212f56c635ea10b",
                fragments: &[
                    (0x986010, 0x986048, 0x1198528, "0106020006520230"),
                    (
                        0x986048,
                        0x98611d,
                        0x11db110,
                        "2177060077e40a007264080005740900106098004860980028851901",
                    ),
                    (
                        0x98611d,
                        0x986148,
                        0x11db12c,
                        "2100020000740900106098004860980028851901",
                    ),
                    (
                        0x986148,
                        0x98614e,
                        0x11db140,
                        "21000000106098004860980028851901",
                    ),
                ],
            },
            Frame {
                begin: 0xe3e8c0,
                end: 0xe3ea89,
                sha: "0b62d0a082ab63428ccd939968ea57bef27c8178c24f7afaf84861a43a091c88",
                fragments: &[(
                    0xe3e8c0,
                    0xe3ea89,
                    0x11c0cfc,
                    "011709001764170017541600173415001701120010700000",
                )],
            },
            Frame {
                begin: 0xe413a0,
                end: 0xe416f5,
                sha: "f1e08affb621f79c16e464b3b265371b0211fa52e866cf2b5be3600a23c781d7",
                fragments: &[
                    (0xe413a0, 0xe413ae, 0x123a7a0, "010e02000e016100"),
                    (
                        0xe413ae,
                        0xe4141a,
                        0x123a7a8,
                        "210f06000ff45a0008745e0004346500a013e400ae13e400a0a72301",
                    ),
                    (
                        0xe4141a,
                        0xe41437,
                        0x123a7c4,
                        "211a06001ac45d0010645f0008546000ae13e4001a14e400a8a72301",
                    ),
                    (
                        0xe41437,
                        0xe415b0,
                        0x123a7e0,
                        "2118040018e45b0008d45c001a14e4003714e400c4a72301",
                    ),
                    (
                        0xe415b0,
                        0xe415f8,
                        0x123a7f8,
                        "2100020000e45b001a14e4003714e400c4a72301",
                    ),
                    (
                        0xe415f8,
                        0xe4163c,
                        0x123a80c,
                        "2100020000645f00ae13e4001a14e400a8a72301",
                    ),
                    (
                        0xe4163c,
                        0xe41657,
                        0x123a820,
                        "21000000ae13e4001a14e400a8a72301",
                    ),
                    (
                        0xe41657,
                        0xe41690,
                        0x123a830,
                        "2100020000f45a00a013e400ae13e400a0a72301",
                    ),
                    (
                        0xe41690,
                        0xe416f5,
                        0x123a844,
                        "21000000a013e400ae13e400a0a72301",
                    ),
                ],
            },
            Frame {
                begin: 0xe3e430,
                end: 0xe3e4ea,
                sha: "db0b1e3ded67b7265ab5ec13824bb1dc8edd0be4ef54233a1c02081469d634f8",
                fragments: &[
                    (
                        0xe3e430,
                        0xe3e48b,
                        0x11c7038,
                        "010c06000c340a000c32087007600650",
                    ),
                    (
                        0xe3e48b,
                        0xe3e4d6,
                        0x123a3f0,
                        "210a04000af4090005e4080030e4e3008be4e30038701c01",
                    ),
                    (
                        0xe3e4d6,
                        0xe3e4ea,
                        0x123a408,
                        "2100000030e4e3008be4e30038701c01",
                    ),
                ],
            },
            Frame {
                begin: 0xe421d0,
                end: 0xe42304,
                sha: "858db66268a26bb553d9142b81b2308d86c3d9dd28c2b7a8b0b9845f97acdb05",
                fragments: &[
                    (
                        0xe421d0,
                        0xe42200,
                        0x117f4ac,
                        "010f06000f6409000f5408000f320b70",
                    ),
                    (
                        0xe42200,
                        0xe422f1,
                        0x123a6d0,
                        "210e04000ee4070005340600d021e4000022e400acf41701",
                    ),
                    (
                        0xe422f1,
                        0xe42304,
                        0x123a6e8,
                        "21000000d021e4000022e400acf41701",
                    ),
                ],
            },
            Frame {
                begin: 0xe3f280,
                end: 0xe3f336,
                sha: "6db2982005e701b2e2914b537f2c9bbfbc777ec26ca18d60e7acd94c31fa0835",
                fragments: &[(
                    0xe3f280,
                    0xe3f336,
                    0x123a3a0,
                    "011d0a001d640b001d540a001d3408001d3219f017e01570",
                )],
            },
            Frame {
                begin: 0xe3f340,
                end: 0xe3f404,
                sha: "bda37c3c6d0fd0402a4a149f3dbb654459aeae3da1795896215687ba579756c6",
                fragments: &[(
                    0xe3f340,
                    0xe3f404,
                    0x122c52c,
                    "01150800157408001564070015340600153211e0",
                )],
            },
            Frame {
                begin: 0x9862e0,
                end: 0x98631c,
                sha: "13d6ae231959dd23022d636f57b0e0e0e09af6015863e528afc1a8d59ecc27a6",
                fragments: &[(0x9862e0, 0x98631c, 0x11c0b10, "0106020006320230")],
            },
            Frame {
                begin: 0x9857c0,
                end: 0x9857eb,
                sha: "4a5bd584432e8274c25ddc454b4c72acdca0434ba3bbb4d86fa665b18c21d7e6",
                fragments: &[(0x9857c0, 0x9857eb, 0x11c0b10, "0106020006320230")],
            },
        ];
        const BLOCKS: &[(u32, &[u8])] = &[
            (
                0xe3e4d6,
                &[
                    0x48, 0x8b, 0xce, 0x48, 0x8b, 0x5c, 0x24, 0x50, 0x48, 0x83, 0xc4, 0x20, 0x5f,
                    0x5e, 0x5d, 0xe9, 0xc6, 0xf8, 0xff, 0xff,
                ],
            ),
            (
                0xe3e902,
                &[
                    0x48, 0x8b, 0x17, 0x33, 0xc0, 0xf0, 0x48, 0x0f, 0xb1, 0x9a, 0x80, 0x01, 0x00,
                    0x00, 0x0f, 0x85, 0x40, 0x01, 0x00, 0x00,
                ],
            ),
            (
                0x985f9e,
                &[
                    0x4c, 0x8b, 0xc0, 0x48, 0x85, 0xc0, 0x74, 0x14, 0xc6, 0x40, 0xf5, 0x01, 0x48,
                    0x8d, 0x05, 0x67, 0x64, 0x6e, 0x00, 0x49, 0x89, 0x00, 0x49, 0x89, 0x78, 0x08,
                    0xeb, 0x03, 0x45, 0x33, 0xc0,
                ],
            ),
            (
                0x985fbd,
                &[
                    0x0f, 0x28, 0x44, 0x24, 0x20, 0x8d, 0x55, 0xff, 0x48, 0x8b, 0x5c, 0x24, 0x48,
                    0x48, 0x8d, 0x05, 0xff, 0x42, 0x38, 0x02, 0x48, 0x8b, 0x6c, 0x24, 0x50, 0x49,
                    0x8d, 0x48, 0xf8, 0x48, 0x89, 0x77, 0x10, 0x48, 0x8b, 0x74, 0x24, 0x58, 0x4c,
                    0x89, 0x47, 0x08, 0x0f, 0x11, 0x47, 0x18, 0x48, 0xc7, 0x01, 0x00, 0x00, 0x00,
                    0x00, 0x48, 0xc1, 0xe2, 0x05, 0x48, 0x03, 0xd0, 0x48, 0x8b, 0x42, 0x08, 0x4c,
                    0x89, 0x00, 0x48, 0x89, 0x4a, 0x08, 0x48, 0x83, 0xc4, 0x30, 0x5f, 0xc3,
                ],
            ),
            (
                0x986010,
                &[
                    0x40, 0x53, 0x48, 0x83, 0xec, 0x30, 0x45, 0x33, 0xdb, 0x8d, 0x59, 0xff, 0x48,
                    0xc1, 0xe3, 0x05, 0x48, 0x8d, 0x05, 0xa9, 0x42, 0x38, 0x02, 0x48, 0x03, 0xd8,
                    0x4c, 0x89, 0x5c, 0x24, 0x20, 0x4c, 0x8d, 0x44, 0x24, 0x20, 0x41, 0xb2, 0x01,
                    0x45, 0x8b, 0xcb, 0x4c, 0x89, 0x44, 0x24, 0x28, 0x4c, 0x39, 0x1b, 0x0f, 0x84,
                    0x00, 0x01, 0x00, 0x00,
                ],
            ),
            (
                0x986050,
                &[
                    0x48, 0x8b, 0x0b, 0x48, 0x8b, 0x41, 0xf8, 0x48, 0x8d, 0x51, 0xf8, 0x48, 0x89,
                    0x03, 0x48, 0x85, 0xc0, 0x75, 0x04, 0x48, 0x89, 0x5b, 0x08, 0x44, 0x39, 0x59,
                    0xe8, 0x75, 0x30, 0x48, 0x8b, 0x41, 0x08, 0x4c, 0x8b, 0x40, 0x10, 0x4d, 0x85,
                    0xc9, 0x75, 0x05, 0x4d, 0x8b, 0xc8, 0xeb, 0x0b, 0x4d, 0x3b, 0xc1, 0x45, 0x0f,
                    0xb6, 0xd2, 0x45, 0x0f, 0x45, 0xd3, 0x4c, 0x89, 0x1a, 0x4c, 0x8b, 0xc2, 0x48,
                    0x8b, 0x44, 0x24, 0x28, 0x48, 0x89, 0x08, 0x48, 0x89, 0x54, 0x24, 0x28, 0x4c,
                    0x39, 0x1b, 0x75, 0xae,
                ],
            ),
            (
                0x9860a2,
                &[
                    0x48, 0x8b, 0x7c, 0x24, 0x20, 0x48, 0x85, 0xff, 0x0f, 0x84, 0x93, 0x00, 0x00,
                    0x00, 0x45, 0x84, 0xd2, 0x75, 0x68,
                ],
            ),
            (
                0x9860d3,
                &[
                    0x48, 0x8b, 0x46, 0x08, 0x48, 0x8b, 0x48, 0x10, 0x48, 0x8b, 0x79, 0x08, 0x80,
                    0x7f, 0x20, 0x00, 0x75, 0x0c, 0x48, 0x8b, 0xcf, 0xe8, 0x93, 0x86, 0x4b, 0x00,
                    0xc6, 0x47, 0x20, 0x01, 0x4c, 0x8d, 0x46, 0xf8, 0x48, 0x8b, 0xd6, 0x48, 0x8b,
                    0xcf, 0xe8, 0xc0, 0x87, 0x4b, 0x00,
                ],
            ),
            (
                0x98611d,
                &[
                    0x49, 0x8b, 0x59, 0x08, 0x44, 0x38, 0x5b, 0x20, 0x75, 0x11, 0x48, 0x8b, 0xcb,
                    0xe8, 0x51, 0x86, 0x4b, 0x00, 0xc6, 0x43, 0x20, 0x01, 0x4c, 0x8b, 0x44, 0x24,
                    0x28, 0x48, 0x8b, 0xd7, 0x48, 0x8b, 0xcb, 0xe8, 0x7d, 0x87, 0x4b, 0x00,
                ],
            ),
            (
                0xe3e8c0,
                &[
                    0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x6c, 0x24, 0x18, 0x48, 0x89, 0x74,
                    0x24, 0x20, 0x57, 0x48, 0x81, 0xec, 0x90, 0x00, 0x00, 0x00, 0x48, 0x8b, 0xf9,
                    0x49, 0x8b, 0xf0, 0x8b, 0x0d, 0xad, 0xc8, 0x73, 0x02, 0x48, 0x8b, 0xea, 0xff,
                    0x15, 0xe4, 0x8e, 0x15, 0x00, 0x4c, 0x8b, 0x0f, 0x48, 0x8b, 0xd8, 0x4d, 0x8b,
                    0x89, 0x80, 0x01, 0x00, 0x00, 0x4d, 0x85, 0xc9, 0x0f, 0x85, 0x54, 0x01, 0x00,
                    0x00,
                ],
            ),
            (
                0xe3e983,
                &[
                    0x4c, 0x8b, 0xc6, 0x48, 0x8b, 0xd5, 0x48, 0x8b, 0xcb, 0xe8, 0x0f, 0x2a, 0x00,
                    0x00,
                ],
            ),
            (
                0xe3ea56,
                &[
                    0x48, 0x8b, 0x0f, 0x48, 0x8d, 0x43, 0x68, 0x45, 0x33, 0xc9, 0x48, 0x89, 0x44,
                    0x24, 0x20, 0x4c, 0x8b, 0xc6, 0x48, 0x8b, 0xd5, 0xe8, 0xc0, 0xf9, 0xff, 0xff,
                ],
            ),
            (
                0xe413a0,
                &[
                    0x48, 0x8b, 0xc4, 0x4c, 0x89, 0x40, 0x18, 0x48, 0x81, 0xec, 0x08, 0x03, 0x00,
                    0x00, 0x48, 0x89, 0x58, 0x20, 0x48, 0x89, 0x78, 0xe8, 0x48, 0x8b, 0xfa, 0x4c,
                    0x89, 0x78, 0xc8, 0x48, 0x8d, 0x42, 0xf8, 0x4c, 0x8b, 0xf9, 0x49, 0x3b, 0xc0,
                    0x75, 0x51, 0xba, 0x01, 0x00, 0x00, 0x00, 0xe8, 0x3d, 0x0f, 0x00, 0x00, 0x48,
                    0x8b, 0xd7, 0x49, 0x8b, 0xcf, 0x48, 0x8b, 0xd8, 0xe8, 0xef, 0x0d, 0x00, 0x00,
                ],
            ),
            (
                0xe41475,
                &[
                    0x48, 0x8b, 0x47, 0xf8, 0x48, 0x8b, 0xd7, 0x49, 0x8b, 0xcf, 0x48, 0x89, 0x84,
                    0x24, 0x18, 0x03, 0x00, 0x00, 0xe8, 0x44, 0x0d, 0x00, 0x00,
                ],
            ),
            (
                0xe3e430,
                &[
                    0x48, 0x89, 0x5c, 0x24, 0x18, 0x55, 0x56, 0x57, 0x48, 0x83, 0xec, 0x20, 0x48,
                    0x8b, 0x05, 0x1d, 0x86, 0x45, 0x00, 0x49, 0x8b, 0xe8, 0x48, 0x8b, 0xda, 0x48,
                    0x8b, 0xf1, 0x48, 0x85, 0xc0, 0x74, 0x09, 0x48, 0x81, 0xc1, 0x90, 0x00, 0x00,
                    0x00, 0xff, 0xd0, 0x48, 0x8d, 0x7b, 0xf8, 0xc6, 0x43, 0xf4, 0x02, 0x45, 0x33,
                    0xc0, 0x48, 0x8d, 0x8e, 0x90, 0x00, 0x00, 0x00, 0x48, 0x8b, 0xd3, 0x48, 0x3b,
                    0xfd, 0x75, 0x10, 0x80, 0x4b, 0xf5, 0x10, 0x4c, 0x8b, 0x4c, 0x24, 0x60, 0xe8,
                    0xfe, 0x0d, 0x00, 0x00, 0xeb, 0x52,
                ],
            ),
            (
                0xe3e484,
                &[
                    0x48, 0x8b, 0x3f, 0x80, 0x4b, 0xf5, 0x10, 0x4c, 0x89, 0x74, 0x24, 0x40, 0x4c,
                    0x89, 0x7c, 0x24, 0x48, 0x4c, 0x8b, 0x7c, 0x24, 0x60, 0x4d, 0x8b, 0xcf, 0xe8,
                    0xde, 0x0d, 0x00, 0x00, 0x48, 0x8b, 0xd7, 0x48, 0x8d, 0x5f, 0xf8, 0x48, 0x8b,
                    0x7f, 0xf8, 0x48, 0x8d, 0x8e, 0x90, 0x00, 0x00, 0x00, 0x4d, 0x8b, 0xcf, 0x45,
                    0x33, 0xc0, 0xc6, 0x42, 0xf4, 0x02, 0x80, 0x4a, 0xf5, 0x10, 0xe8, 0xb9, 0x0d,
                    0x00, 0x00, 0x48, 0x3b, 0xdd, 0x75, 0xd6, 0x4c, 0x8b, 0x7c, 0x24, 0x48, 0x4c,
                    0x8b, 0x74, 0x24, 0x40,
                ],
            ),
            (
                0xe3f28f,
                &[
                    0x48, 0x89, 0x54, 0x24, 0x10, 0x57, 0x41, 0x56, 0x41, 0x57, 0x48, 0x83, 0xec,
                    0x20, 0x49, 0x63, 0xc0, 0x4d, 0x8b, 0xf1, 0x4c, 0x8b, 0xf9, 0x48, 0x8d, 0x2c,
                    0xc1,
                ],
            ),
            (
                0xe3f2f2,
                &[
                    0x48, 0x8d, 0x54, 0x24, 0x48, 0x48, 0x8b, 0xcb, 0xe8, 0x41, 0x00, 0x00, 0x00,
                ],
            ),
            (
                0xe3f3d0,
                &[
                    0x48, 0x8b, 0x43, 0x08, 0x83, 0xe7, 0x01, 0x48, 0x8b, 0x04, 0x30, 0x48, 0x8d,
                    0x0c, 0xf8, 0x48, 0x85, 0xc9, 0x74, 0x06, 0x49, 0x8b, 0x06, 0x48, 0x89, 0x01,
                    0x48, 0xff, 0x43, 0x20,
                ],
            ),
            (
                0xe421d0,
                &[
                    0x48, 0x89, 0x6c, 0x24, 0x18, 0x48, 0x89, 0x74, 0x24, 0x20, 0x57, 0x48, 0x83,
                    0xec, 0x20, 0x0f, 0xb7, 0x6a, 0xf6, 0x48, 0x8b, 0xf2, 0xc6, 0x42, 0xf4, 0x02,
                    0x48, 0x8b, 0xf9, 0x66, 0x85, 0xed, 0x0f, 0x84, 0xfb, 0x00, 0x00, 0x00, 0x66,
                    0x3b, 0x69, 0x40, 0x0f, 0x84, 0xf1, 0x00, 0x00, 0x00,
                ],
            ),
            (
                0xe4228d,
                &[
                    0x0f, 0xb7, 0xc5, 0xf7, 0xd8, 0x48, 0x63, 0xc8, 0x48, 0x8b, 0xc6, 0x48, 0x83,
                    0xc8, 0x03, 0x48, 0xc1, 0xe1, 0x07, 0x48, 0x03, 0x4f, 0x20, 0x48, 0x89, 0x4b,
                    0x18, 0x48, 0x89, 0x43, 0x08, 0x48, 0x8b, 0x46, 0xc8, 0x48, 0x89, 0x43, 0xc8,
                ],
            ),
            (
                0xe422c2,
                &[
                    0x48, 0x8b, 0x4b, 0x18, 0x48, 0x8d, 0x53, 0x10, 0x4c, 0x89, 0x32, 0x48, 0x8b,
                    0xc3, 0x4c, 0x8b, 0x74, 0x24, 0x38, 0x48, 0x87, 0x51, 0x08, 0x48, 0x89, 0x1a,
                ],
            ),
            (
                0x9862e0,
                &[
                    0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0x41, 0x08, 0x48, 0x8b, 0xd9,
                    0x33, 0xc9, 0x48, 0x89, 0x48, 0x08, 0x48, 0x8b, 0x43, 0x08, 0x48, 0x89, 0x48,
                    0x10, 0x48, 0x8d, 0x0d, 0x3e, 0x3f, 0x38, 0x02, 0xe8, 0x99, 0x67, 0x00, 0x00,
                    0x48, 0x8b, 0x4b, 0x08, 0x8b, 0xd0, 0x4c, 0x8b, 0x01, 0x41, 0xff, 0x50, 0x08,
                    0x33, 0xc0, 0x48, 0x83, 0xc4, 0x20, 0x5b, 0xc3,
                ],
            ),
            (
                0x9857c0,
                &[
                    0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8d, 0x05, 0xe3, 0x6b, 0x6e, 0x00,
                    0x48, 0x8b, 0xd9, 0x48, 0x89, 0x01, 0xf6, 0xc2, 0x01, 0x74, 0x0a, 0xba, 0x10,
                    0x00, 0x00, 0x00, 0xe8, 0x2e, 0x47, 0x7d, 0xff, 0x48, 0x8b, 0xc3, 0x48, 0x83,
                    0xc4, 0x20, 0x5b, 0xc3,
                ],
            ),
        ];
        fn hex(b: &[u8]) -> String {
            b.iter().map(|x| format!("{x:02x}")).collect()
        }
        fn frame(pe: &Pe<'_>, s: &Frame) -> Result<Value, String> {
            let length = s
                .end
                .checked_sub(s.begin)
                .filter(|n| *n > 0)
                .ok_or("pending reversed/empty frame")?;
            let mut next = s.begin;
            let mut parts = Vec::new();
            for &(begin, end, unwind, expected) in s.fragments {
                if begin != next || end <= begin || end > s.end {
                    return Err("pending incomplete/overlapping fragment partition".into());
                }
                let f = Function { begin, end, unwind };
                if pe.function(begin) != Some(f) || pe.function(end - 1) != Some(f) {
                    return Err("pending exact pdata fragment differs".into());
                }
                let chain = pe.chain(f)?;
                if chain.last().is_none_or(|p| p.begin != s.begin)
                    || chain.iter().any(|p| {
                        !s.fragments.iter().any(|&(a, z, u, _)| {
                            *p == (Function {
                                begin: a,
                                end: z,
                                unwind: u,
                            })
                        })
                    })
                {
                    return Err("pending CHAININFO owner/partition differs".into());
                }
                let h = pe.span(unwind, 4, false)?;
                let aligned = 4 + ((usize::from(h[2]) + 1) & !1) * 2;
                let flags = h[0] >> 3;
                let total = aligned
                    + if flags & 4 != 0 {
                        12
                    } else if flags & 3 != 0 {
                        4
                    } else {
                        0
                    };
                let codes = pe.span(unwind, total, false)?;
                if hex(codes) != expected || u32::from(h[1]) > end - begin {
                    return Err("pending complete unwind/prologue differs".into());
                }
                parts.push(json!({"begin":begin,"end":end,"unwind":unwind,
                    "span_sha256":format!("{:x}",Sha256::digest(pe.span(begin,(end-begin)as usize,true)?)),
                    "unwind_bytes_hex":hex(codes),
                    "fragment_prologue_bytes_hex":hex(pe.span(begin,usize::from(h[1]),true)?),
                    "chain":chain.iter().map(|p|json!({"begin":p.begin,"end":p.end,"unwind":p.unwind})).collect::<Vec<_>>()}));
                next = end;
            }
            if next != s.end
                || format!(
                    "{:x}",
                    Sha256::digest(pe.span(s.begin, length as usize, true)?)
                ) != s.sha
            {
                return Err("pending complete frame SHA/partition differs".into());
            }
            Ok(
                json!({"begin_rva":s.begin,"end_rva":s.end,"fullbytes_sha256":s.sha,
                "span_type":"contiguous complete pdata/CHAININFO fragment partition",
                "fragments":parts,"scope":"fixed disk bytes and unwind metadata; not observed execution or lifetime"}),
            )
        }
        fn blocks(pe: &Pe<'_>) -> Result<Vec<Value>, String> {
            BLOCKS
                .iter()
                .map(|&(r, b)| pe.witness(r, b, false))
                .collect()
        }
        fn references(pe: &Pe<'_>) -> Result<Vec<Value>, String> {
            let mut out = Vec::new();
            for (r, op, target) in [
                (0x985f99, 0xe8, 0xe3ccf0),
                (0x9860fb, 0xe8, 0xe3e8c0),
                (0x98613e, 0xe8, 0xe3e8c0),
                (0xe3e98c, 0xe8, 0xe413a0),
                (0xe3ea6b, 0xe8, 0xe3e430),
                (0xe413dc, 0xe8, 0xe421d0),
                (0xe41487, 0xe8, 0xe421d0),
                (0xe3e47d, 0xe8, 0xe3f280),
                (0xe3e49d, 0xe8, 0xe3f280),
                (0xe3e4c2, 0xe8, 0xe3f280),
                (0xe3f2fa, 0xe8, 0xe3f340),
                (0xe3e4e5, 0xe9, 0xe3ddb0),
                (0x9857dd, 0xe8, 0x159f10),
            ] {
                let actual = rel_target(r, pe.span(r, 5, true)?, op, 5, 1)?;
                if actual != target {
                    return Err("pending computed call/tail target differs".into());
                }
                pe.span(actual, 1, true)?;
                out.push(
                    json!({"rva":r,"kind":if op==0xe8 {"direct_call"}else{"direct_tail"},
                    "computed_target_rva":actual}),
                );
            }
            // Both branch operands must reach the same fallback entry.
            for r in [0xe3e8fc, 0xe3e910] {
                let b = pe.span(r, 6, true)?;
                if b[..2] != [0x0f, 0x85] || rel_target(r, b, 0x0f, 6, 2)? != 0xe3ea56 {
                    return Err("pending fallback JNE operand differs".into());
                }
                out.push(json!({"rva":r,"kind":"conditional_JNE","computed_target_rva":0xe3ea56}));
            }
            // Fixed preferred-image pointers are data, never indirect-call execution evidence.
            for (slot, target) in [(0x106c418, 0x9857c0), (0x106c420, 0x9862e0)] {
                let raw = u64_at(pe.span(slot, 8, false)?, 0)?;
                if raw.checked_sub(pe.base) != Some(u64::from(target)) {
                    return Err("pending preferred-image table pointer differs".into());
                }
                pe.span(target, 1, true)?;
                out.push(json!({"slot_rva":slot,"preferred_va":raw,"computed_target_rva":target,
                    "scope":"fixed disk pointer at preferred ImageBase; no DIR64 or executed target claim"}));
            }
            let b = pe.span(0x985faa, 7, true)?;
            if b[..3] != [0x48, 0x8d, 0x05] || rel_target(0x985faa, b, 0x48, 7, 3)? != 0x106c418 {
                return Err("pending T table RIP operand differs".into());
            }
            pe.span(0x106c418, 16, false)?;
            out.push(json!({"rva":0x985faa,"kind":"rip_relative_table_address",
                "computed_target_rva":0x106c418}));
            Ok(out)
        }
        pub(super) fn inspect(pe: &Pe<'_>) -> Result<Value, String> {
            let frames = FRAMES
                .iter()
                .map(|s| frame(pe, s))
                .collect::<Result<Vec<_>, _>>()?;
            let blocks = blocks(pe)?;
            let refs = references(pe)?;
            Ok(json!({"schema":"todo880-pending-publication-static-v2",
                "host_sha256":HOST_SHA,"frames":frames,"contiguous_blocks":blocks,"references":refs,
                "input_handoff":{
                    "construction":"985F9E R8=allocation RAX; nonzero branch writes T.vft=106C418 and T+8=original RDI P at985FB4",
                    "pending_link":"985FD6 RCX=T-8; 985FE3 [P+8]=T; 985FFD writes T to the prior list tail and986000 writes T-8 as new tail",
                    "publication_general":"9860D3..986100 obtains arena receiver via [T+8]->+10->+8; RDX=T and R8=T-8 at call9860FB",
                    "publication_same_receiver":"98611D..986143 loads receiver [R9+8]; RDX=RDI head T and R8=stack-held tail at98613E",
                    "argument_lifetime_limit":"conditional on preceding calls returning normally and preserving Win64 nonvolatile registers and caller-owned stack slots; native pointer values only, no runtime identity/lifetime or cancellation inferred"
                },
                "branches":[
                    {"caller":0xe3e98c,"callee":0xe413a0,
                     "arguments":"E3E8C0 saves entry RDX in RBP and R8 in RSI; E3E983..E3E991 restores R8=RSI,RDX=RBP,RCX=RBX",
                     "transport":"E413B6 RDI=RDX; compares RDX-8 with R8; single and linked-element paths pass RDX element to E421D0",
                     "saved_pointer":"E421D0 saves RDX in RSI; normal zero/equal-word branch returns original RSI atE422F6; alternate constructs [new+8]=RSI|3 atE422A8",
                     "publication_store":"E413EC single or E41586/E415C3 multi-element copies E421D0 results to pointer array; execution is not shown here"},
                    {"caller":0xe3ea6b,"callee":0xe3e430,
                     "arguments":"E3EA56..E3EA70 restores R8=RSI,RDX=RBP; RCX=[entry RCX],R9d=0,stack fifth argument=RBX+68",
                     "transport":"E3E443 RBP=R8 tail; E3E446 RBX=RDX element; E3E47D/E3E49D/E3E4C2 call E3F280 with current element in RDX",
                     "saved_pointer":"E3F28F saves input RDX to home slot; E3F2F2 passes address of same slot to E3F340; E3F3E4..E3F3E7 stores its qword in selected pointer slot",
                     "tail":"E3E4E5 direct tail to E3DDB0; target execution body/receiver ownership not inferred"}
                ],
                "execute_and_destroy":{
                    "table_rva":0x106c418,"execute_slot":8,"execute_target":0x9862e0,
                    "execute_input":"RCX=T;9862E6 loads [T+8];986307 reloads it as RCX;986310 calls [[P]+8] with EDX from98CAA0; normal RET98631B",
                    "destroy_slot":0,"destroy_target":0x9857c0,
                    "destroy_bytes":"9857C0 resets [RCX] vft; TEST DL,1 conditional path calls159F10 with EDX16 and RCX preserved; no load/dereference of T+8 in complete frame",
                    "scope":"disk table targets and callee bodies only; queue consumer-to-virtual-execute/destructor call edge not established"
                },
                "not_established_by_this_audit":[
                    "runtime T/P identity or lifetime through scheduler copies",
                    "queue dequeue-to-9862E0 connection and typed/tagged pointer handling",
                    "unexecuted pending T cancellation, destructor invocation or P settlement",
                    "state byte or word meaning; zero/FreeList as Cancel or join",
                    "same-set Wait joining every pending/copied T",
                    "World epoch authority, generation permits and all-domain Close/Rebuild"],
                "certificate_issued":false,"structural_declarations":{"runtime_calls":0,"game_runs":0,
                    "basis":"borrowed file buffers only; zero values are structural declarations, not native measurements"}}))
        }
        #[cfg(test)]
        mod tests {
            use super::*;
            fn image() -> Vec<u8> {
                let mut b = vec![0u8; 0x1240000];
                for &(r, code) in BLOCKS {
                    let p = (r - 0xc00) as usize;
                    b[p..p + code.len()].copy_from_slice(code);
                }
                // One call just before the fixed contiguous allocation-result block.
                b[0x985f99 - 0xc00..0x985f9e - 0xc00].copy_from_slice(&[0xe8, 0x52, 0x6d, 0x4b, 0]);
                for (slot, target) in [(0x106c418, 0x9857c0), (0x106c420, 0x9862e0)] {
                    let p = slot - 0xc00;
                    b[p..p + 8].copy_from_slice(&(0x140000000u64 + target).to_le_bytes());
                }
                b
            }
            fn pe(b: &[u8]) -> Pe<'_> {
                Pe {
                    image: b,
                    base: 0x140000000,
                    sections: vec![
                        Section {
                            name: *b".text\0\0\0",
                            rva: 0x1000,
                            virtual_size: 0xf95000,
                            raw: 0x400,
                            size: 0xf95000,
                            flags: 0x60000020,
                        },
                        Section {
                            name: *b".rdata\0\0",
                            rva: 0xf96000,
                            virtual_size: 0x2a9000,
                            raw: 0xf95400,
                            size: 0x2a9000,
                            flags: 0x40000040,
                        },
                    ],
                    functions: vec![],
                    relocations: [0x106c418, 0x106c420].into_iter().collect(),
                }
            }
            #[test]
            fn pending_publication_call_operands_and_argument_blocks_reject_mutations() {
                let mut b = image();
                blocks(&pe(&b)).unwrap();
                references(&pe(&b)).unwrap();
                for r in [
                    0x9860fb, 0x98613e, 0xe3e98c, 0xe3ea6b, 0xe413dc, 0xe41487, 0xe3e47d, 0xe3e49d,
                    0xe3e4c2, 0xe3f2fa,
                ] {
                    let p = r - 0xc00 + 1;
                    b[p] ^= 1;
                    assert!(references(&pe(&b)).is_err(), "operand {r:x}");
                    b[p] ^= 1;
                }
                for r in [
                    0x985fb4, 0x9860f5, 0xe3e8da, 0xe3e8e3, 0xe3e983, 0xe3ea68, 0xe3e446, 0xe3f28f,
                    0xe3f3e4, 0xe422a8,
                ] {
                    let p = r - 0xc00;
                    b[p] ^= 1;
                    assert!(blocks(&pe(&b)).is_err(), "argument {r:x}");
                    b[p] ^= 1;
                }
                b[0xe3e910 - 0xc00 + 2] ^= 1;
                assert!(references(&pe(&b)).is_err());
                b[0xe3e910 - 0xc00 + 2] ^= 1;
                b[0x106c420 - 0xc00] ^= 1;
                assert!(references(&pe(&b)).is_err(), "wrong execute table target");
            }
            #[test]
            fn pending_publication_code_data_and_relative_bounds_reject() {
                let b = image();
                let mut mapped = pe(&b);
                mapped.sections[0].flags &= !0x20000000;
                assert!(blocks(&mapped).is_err());
                let mut mapped = pe(&b);
                mapped.sections[1].flags |= 0x20000000;
                assert!(references(&mapped).is_err());
                let mut mapped = pe(&b);
                mapped.relocations.clear();
                assert!(references(&mapped).is_ok());
                assert!(
                    mapped.pointer(0x106c418, 0x9857c0).is_err(),
                    "DLL relocated-pointer validation remains strict"
                );
                assert!(references(&pe(&b[..0x106c424 - 0xc00])).is_err());
                assert!(rel_target(0, &[0xe8, 0xf0, 0xff, 0xff, 0xff], 0xe8, 5, 1).is_err());
                assert!(rel_target(u32::MAX, &[0xe8, 0, 0, 0, 0], 0xe8, 5, 1).is_err());
            }
            #[test]
            fn pending_publication_executable_without_relocations_does_not_relax_dll_checks() {
                let mut b = vec![0; 0x800];
                b[..2].copy_from_slice(b"MZ");
                b[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
                b[0x80..0x84].copy_from_slice(b"PE\0\0");
                for (p, v) in [
                    (0x84, 0x8664u16),
                    (0x86, 2),
                    (0x94, 0xf0),
                    (0x96, 2),
                    (0x98, 0x20b),
                ] {
                    b[p..p + 2].copy_from_slice(&v.to_le_bytes());
                }
                b[0xb0..0xb8].copy_from_slice(&0x140000000u64.to_le_bytes());
                for (p, v) in [(0x104, 6u32), (0x120, 0x2000), (0x124, 12)] {
                    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
                }
                for (p, name, rva, raw, flags) in [
                    (0x188, *b".text\0\0\0", 0x1000u32, 0x400u32, 0x60000020u32),
                    (0x1b0, *b".rdata\0\0", 0x2000, 0x600, 0x40000040),
                ] {
                    b[p..p + 8].copy_from_slice(&name);
                    for (offset, v) in [(8, 0x100), (12, rva), (16, 0x100), (20, raw), (36, flags)]
                    {
                        b[p + offset..p + offset + 4].copy_from_slice(&v.to_le_bytes());
                    }
                }
                for (p, v) in [(0x600, 0x1000u32), (0x604, 0x1008), (0x608, 0x2010)] {
                    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
                }
                b[0x610] = 1;
                assert!(Pe::parse_executable(&b).is_ok());
                assert!(
                    Pe::parse(&b).is_err(),
                    "Core relocation requirement is unchanged"
                );
                b[0x130..0x134].copy_from_slice(&0x2020u32.to_le_bytes());
                assert!(
                    Pe::parse_executable(&b).is_err(),
                    "one-sided absent directory is malformed"
                );
                b[0x130..0x134].fill(0);
                b[0x134..0x138].copy_from_slice(&8u32.to_le_bytes());
                assert!(
                    Pe::parse_executable(&b).is_err(),
                    "reverse one-sided directory is malformed"
                );
                b[0x134..0x138].fill(0);
                b[0x96..0x98].copy_from_slice(&0x2002u16.to_le_bytes());
                assert!(
                    Pe::parse_executable(&b).is_err(),
                    "DLL cannot use the executable policy"
                );
            }
            #[test]
            fn pending_publication_full_pdata_chain_partition_sha_and_unwind_reject() {
                let mut b = vec![0u8; 0x1100];
                b[0x400..0x410].fill(0x90);
                b[0x1000..0x1004].copy_from_slice(&[1, 0, 0, 0]);
                b[0x1010..0x1020]
                    .copy_from_slice(&[0x21, 0, 0, 0, 0, 0x10, 0, 0, 8, 0x10, 0, 0, 0, 0x20, 0, 0]);
                fn mk(b: &[u8]) -> Pe<'_> {
                    Pe {
                        image: b,
                        base: 0x140000000,
                        sections: vec![
                            Section {
                                name: *b".text\0\0\0",
                                rva: 0x1000,
                                virtual_size: 16,
                                raw: 0x400,
                                size: 16,
                                flags: 0x60000020,
                            },
                            Section {
                                name: *b".rdata\0\0",
                                rva: 0x2000,
                                virtual_size: 32,
                                raw: 0x1000,
                                size: 32,
                                flags: 0x40000040,
                            },
                        ],
                        functions: vec![
                            Function {
                                begin: 0x1000,
                                end: 0x1008,
                                unwind: 0x2000,
                            },
                            Function {
                                begin: 0x1008,
                                end: 0x1010,
                                unwind: 0x2010,
                            },
                        ],
                        relocations: Default::default(),
                    }
                }
                let s = Frame {
                    begin: 0x1000,
                    end: 0x1010,
                    sha: "87e36ce5c70a77d0b4bafbe2d6d1f53167344236134dab16808ae0cdd505c54d",
                    fragments: &[
                        (0x1000, 0x1008, 0x2000, "01000000"),
                        (0x1008, 0x1010, 0x2010, "21000000001000000810000000200000"),
                    ],
                };
                frame(&mk(&b), &s).unwrap();
                b[0x40f] ^= 1;
                assert!(frame(&mk(&b), &s).is_err());
                b[0x40f] ^= 1;
                b[0x1014] ^= 1;
                assert!(frame(&mk(&b), &s).is_err());
                b[0x1014] ^= 1;
                b[0x1001] = 9;
                assert!(frame(&mk(&b), &s).is_err());
                b[0x1001] = 0;
                let mut mapped = mk(&b);
                mapped.functions[1].end = 0x1011;
                assert!(frame(&mapped, &s).is_err());
                let wrong = Frame { end: 0x100f, ..s };
                assert!(frame(&mk(&b), &wrong).is_err());
                assert!(frame(&mk(&b[..0x101f]), &s).is_err());
            }
        }
    }

    /// Appended to the ordinary fixed-file audit after BOTH original hash gates.
    pub(super) fn inspect_governor_birth(core: &[u8]) -> Result<Value, String> {
        governor_birth::inspect(core)
    }
    mod governor_birth {
        use super::*;
        const GOV_CTOR_PREFIX: &[u8] = &[
            0x48, 0x89, 0x5c, 0x24, 0x08, 0x48, 0x89, 0x74, 0x24, 0x10, 0x48, 0x89, 0x7c, 0x24,
            0x18, 0x55, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57, 0x48, 0x8b, 0xec, 0x48,
            0x83, 0xec, 0x50, 0x48, 0x8b, 0xf9, 0xe8, 0x79, 0xc3, 0xb9, 0xff, 0x48, 0x8d, 0x05,
            0x32, 0x08, 0x56, 0x00, 0x45, 0x33, 0xe4, 0x48, 0x89, 0x07,
        ];
        const GOV_JOB_INIT: &[u8] = &[
            0x48, 0x8d, 0x05, 0x4c, 0xff, 0x51, 0x00, 0x4c, 0x89, 0xa7, 0xd8, 0x00, 0x00, 0x00,
            0x4c, 0x89, 0xa7, 0xe0, 0x00, 0x00, 0x00, 0x48, 0x8d, 0x0d, 0x67, 0x39, 0x55, 0x00,
            0x48, 0x89, 0x87, 0xe8, 0x00, 0x00, 0x00, 0x0f, 0x57, 0xc0, 0x4c, 0x89, 0xa7, 0xf0,
            0x00, 0x00, 0x00, 0x48, 0x8d, 0x05, 0xa7, 0x05, 0x56, 0x00, 0x48, 0x89, 0x87, 0xd0,
            0x00, 0x00, 0x00, 0x48, 0x8d, 0x05, 0x4e, 0x39, 0x55, 0x00, 0x2b, 0xc1, 0x4c, 0x89,
            0xa7, 0xf8, 0x00, 0x00, 0x00, 0xff, 0xc0, 0x66, 0xc7, 0x87, 0x08, 0x01, 0x00, 0x00,
            0x01, 0x00,
        ];
        const GOV_CTOR_RETURN: &[u8] = &[
            0x4c, 0x8d, 0x5c, 0x24, 0x50, 0x48, 0x8b, 0xc7, 0x49, 0x8b, 0x5b, 0x30, 0x49, 0x8b,
            0x73, 0x38, 0x49, 0x8b, 0x7b, 0x40, 0x49, 0x8b, 0xe3, 0x41, 0x5f, 0x41, 0x5e, 0x41,
            0x5d, 0x41, 0x5c, 0x5d, 0xc3,
        ];
        const GOV_OWNER_DROP: &[u8] = &[
            0x48, 0x89, 0x5c, 0x24, 0x08, 0x57, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8d, 0x05, 0xdf,
            0x02, 0x56, 0x00, 0x48, 0x8b, 0xd9, 0x48, 0x89, 0x01, 0x48, 0x8d, 0x05, 0x4a, 0x02,
            0x56, 0x00, 0x48, 0x89, 0x81, 0x90, 0x01, 0x00, 0x00, 0x48, 0x8b, 0x89, 0x98, 0x01,
            0x00, 0x00, 0x48, 0x85, 0xc9, 0x74, 0x0c, 0x48, 0x8d, 0x93, 0x90, 0x01, 0x00, 0x00,
            0xe8, 0x83, 0x0b, 0x4f, 0x00, 0x48, 0x8d, 0x8b, 0xa0, 0x01, 0x00, 0x00, 0xe8, 0x37,
            0xff, 0xff, 0xff, 0x48, 0x8d, 0x8b, 0x90, 0x01, 0x00, 0x00, 0xe8, 0x3b, 0xef, 0x4e,
            0x00, 0x48, 0x8b, 0x8b, 0x30, 0x01, 0x00, 0x00, 0x48, 0x8d, 0x05, 0x7d, 0x01, 0x56,
            0x00, 0x48, 0x89, 0x83, 0x28, 0x01, 0x00, 0x00, 0x48, 0x85, 0xc9, 0x74, 0x0c, 0x48,
            0x8d, 0x93, 0x28, 0x01, 0x00, 0x00, 0xe8, 0x45, 0x0b, 0x4f, 0x00, 0x48, 0x8d, 0x8b,
            0x38, 0x01, 0x00, 0x00, 0xe8, 0x89, 0xfe, 0xff, 0xff, 0x48, 0x8d, 0x8b, 0x28, 0x01,
            0x00, 0x00, 0xe8, 0xfd, 0xee, 0x4e, 0x00, 0x48, 0x8b, 0x8b, 0x18, 0x01, 0x00, 0x00,
            0x48, 0x8d, 0x05, 0xe7, 0x4b, 0x55, 0x00, 0x48, 0x89, 0x83, 0x10, 0x01, 0x00, 0x00,
            0x48, 0x85, 0xc9, 0x74, 0x0c, 0x48, 0x8d, 0x93, 0x10, 0x01, 0x00, 0x00, 0xe8, 0x07,
            0x0b, 0x4f, 0x00, 0x48, 0x8d, 0x8b, 0x10, 0x01, 0x00, 0x00, 0xe8, 0xcb, 0xee, 0x4e,
            0x00, 0x48, 0x8d, 0x8b, 0xd0, 0x00, 0x00, 0x00, 0xe8, 0xdf, 0xbb, 0x4e, 0x00, 0x48,
            0x8d, 0x8b, 0xc0, 0x00, 0x00, 0x00, 0xe8, 0x23, 0xc4, 0x15, 0x00, 0x48, 0x8d, 0x8b,
            0xb0, 0x00, 0x00, 0x00, 0xe8, 0x17, 0xc4, 0x15, 0x00, 0x48, 0x8d, 0x8b, 0xa0, 0x00,
            0x00, 0x00, 0xe8, 0x0b, 0xc4, 0x15, 0x00, 0x48, 0x8d, 0x4b, 0x08, 0xe8, 0x32, 0xf2,
            0x4e, 0x00, 0x48, 0x8b, 0xcb, 0x48, 0x8b, 0x5c, 0x24, 0x30, 0x48, 0x83, 0xc4, 0x20,
            0x5f, 0xe9, 0x30, 0xbd, 0xb9, 0xff,
        ];
        const GOV_JOB_DROP: &[u8] = &[
            0x48, 0x89, 0x5c, 0x24, 0x08, 0x57, 0x48, 0x83, 0xec, 0x20, 0x8b, 0xda, 0x48, 0x8b,
            0xf9, 0xe8, 0x9c, 0xaf, 0x4e, 0x00, 0xf6, 0xc3, 0x01, 0x74, 0x0d, 0xba, 0x40, 0x00,
            0x00, 0x00, 0x48, 0x8b, 0xcf, 0xe8, 0xe2, 0x8c, 0x50, 0x00, 0x48, 0x8b, 0xc7, 0x48,
            0x8b, 0x5c, 0x24, 0x30, 0x48, 0x83, 0xc4, 0x20, 0x5f, 0xc3,
        ];
        const GOV_OWNER_DELETE: &[u8] = &[
            0x48, 0x89, 0x5c, 0x24, 0x08, 0x57, 0x48, 0x83, 0xec, 0x20, 0x8b, 0xda, 0x48, 0x8b,
            0xf9, 0xe8, 0xac, 0xf2, 0xff, 0xff, 0xf6, 0xc3, 0x01, 0x74, 0x0d, 0xba, 0xf0, 0x01,
            0x00, 0x00, 0x48, 0x8b, 0xcf, 0xe8, 0xa2, 0x8c, 0x50, 0x00, 0x48, 0x8b, 0xc7, 0x48,
            0x8b, 0x5c, 0x24, 0x30, 0x48, 0x83, 0xc4, 0x20, 0x5f, 0xc3,
        ];
        const GOV_CREATE: &[u8] = &[
            0x48, 0x83, 0xec, 0x38, 0xe8, 0x97, 0xc2, 0xee, 0xff, 0x41, 0xb9, 0x86, 0x01, 0x00,
            0x00, 0xc7, 0x44, 0x24, 0x28, 0x00, 0x00, 0x00, 0x00, 0x8b, 0xd0, 0xc7, 0x44, 0x24,
            0x20, 0x33, 0x00, 0x00, 0x00, 0x4c, 0x8d, 0x05, 0x68, 0xf4, 0x55, 0x00, 0x41, 0x8d,
            0x49, 0x6a, 0xe8, 0x4f, 0x95, 0x4e, 0x00, 0x48, 0x85, 0xc0, 0x74, 0x0c, 0x48, 0x8b,
            0xc8, 0x48, 0x83, 0xc4, 0x38, 0xe9, 0x6e, 0xeb, 0xff, 0xff, 0x48, 0x83, 0xc4, 0x38,
            0xc3,
        ];
        const GOV_JOB_BASE_RESET: &[u8] = &[
            0x48, 0x8d, 0x05, 0x79, 0x3e, 0x03, 0x00, 0x48, 0x89, 0x01, 0xc3,
        ];

        fn spans(pe: &Pe<'_>) -> Result<Vec<Value>, String> {
            let mut result = Vec::new();
            for (rva, bytes, whole) in [
                (0x4a5f30, GOV_CTOR_PREFIX, false),
                (0x4a606d, GOV_JOB_INIT, false),
                (0x4a638c, GOV_CTOR_RETURN, false),
                (0x4a64a0, GOV_OWNER_DROP, true),
                (0x4a71a0, GOV_JOB_DROP, true),
                (0x4a71e0, GOV_OWNER_DELETE, true),
                (0x4a7380, GOV_CREATE, true),
                (0x992150, GOV_JOB_BASE_RESET, false),
            ] {
                result.push(pe.witness(rva, bytes, whole)?);
            }
            // Separate neighboring table belongs to another destination R15,
            // not to P=RDI+D0. Do not extend the two-method job table into it.
            result.push(pe.witness(
                0x4a600e,
                &[
                    0x48, 0x8d, 0x05, 0x43, 6, 0x56, 0, 0x48, 0x8b, 0x55, 0xe0, 0x49, 0x89, 7,
                ],
                false,
            )?);
            let (_, producer) = MANAGER_FLOW_BLOCKS
                .iter()
                .find(|(r, _)| *r == 0x4a7a27)
                .ok_or("governor existing producer block absent")?;
            result.push(pe.witness(0x4a7a27, producer, false)?);
            Ok(result)
        }
        fn references(pe: &Pe<'_>) -> Result<Vec<Value>, String> {
            let mut result = Vec::new();
            for (rva, op, target) in [
                (0x4a5f52, 0xe8, 0x422d0),
                (0x4a73ac, 0xe8, 0x990900),
                (0x4a73bd, 0xe9, 0x4a5f30),
                (0x4a71af, 0xe8, 0x992150),
                (0x4a71c1, 0xe8, 0x9afea8),
                (0x4a71ef, 0xe8, 0x4a64a0),
                (0x4a7201, 0xe8, 0x9afea8),
                (0x4a656c, 0xe8, 0x992150),
                (0x4a65ab, 0xe9, 0x422e0),
                (0x4a7a4b, 0xe8, 0x4a1100),
                (0x4a7a53, 0xe8, 0x4a10b0),
            ] {
                let actual = rel_target(rva, pe.span(rva, 5, true)?, op, 5, 1)?;
                if actual != target {
                    return Err("governor birth relative call/tail differs".into());
                }
                pe.span(target, 1, true)?;
                result.push(
                    json!({"rva":rva,"kind":if op==0xe8 {"direct_call"} else {"direct_tail"},
                    "computed_target_rva":actual}),
                );
            }
            for (rva, prefix, target) in [
                (0x4a5f57, [0x48, 0x8d, 0x05], 0xa06790),
                (0x4a606d, [0x48, 0x8d, 0x05], 0x9c5fc0),
                (0x4a609a, [0x48, 0x8d, 0x05], 0xa06648),
                (0x4a64aa, [0x48, 0x8d, 0x05], 0xa06790),
                (0x4a600e, [0x48, 0x8d, 0x05], 0xa06658),
                (0x992150, [0x48, 0x8d, 0x05], 0x9c5fd0),
                (0x4a7a27, [0x48, 0x8b, 0x05], 0xb8aa68),
                (0x4a7a3a, [0x4c, 0x8d, 0x05], 0xa06920),
            ] {
                let b = pe.span(rva, 7, true)?;
                if b[..3] != prefix || rel_target(rva, b, prefix[0], 7, 3)? != target {
                    return Err("governor birth RIP reference differs".into());
                }
                if target == 0xb8aa68 {
                    // This mutable singleton is in the loader-zero-filled
                    // tail of .data. Validate its virtual address, never
                    // invent file bytes for it or read a runtime value.
                    let mut sections = pe.sections.iter().filter(|s| {
                        s.name == *b".data\0\0\0"
                            && s.flags & 0xe000_0000 == 0xc000_0000
                            && target >= s.rva
                            && s.rva
                                .checked_add(s.virtual_size)
                                .is_some_and(|end| target < end)
                    });
                    if sections.next().is_none() || sections.next().is_some() {
                        return Err("governor singleton virtual data target differs".into());
                    }
                } else {
                    pe.span_any(target, 1)?;
                }
                result.push(json!({"rva":rva,"kind":"rip_relative_reference",
                    "opcode_bytes":prefix,"computed_target_rva":target}));
            }
            Ok(result)
        }
        fn complete(
            pe: &Pe<'_>,
            begin: u32,
            end: u32,
            unwind: u32,
            expected_sha: &str,
            codes: &[u8],
        ) -> Result<Value, String> {
            let f = pe
                .function(begin)
                .ok_or("governor function pdata missing")?;
            if f != (Function { begin, end, unwind }) || pe.chain(f)?.len() != 1 {
                return Err("governor full function/unwind range differs".into());
            }
            let body = pe.span(begin, (end - begin) as usize, true)?;
            if format!("{:x}", Sha256::digest(body)) != expected_sha {
                return Err("governor complete function SHA differs".into());
            }
            if pe.span(unwind, codes.len(), false)? != codes {
                return Err("governor full unwind bytes differ".into());
            }
            let h = pe.span(unwind, 4, false)?;
            let prologue = pe.span(begin, usize::from(h[1]), true)?;
            Ok(
                json!({"begin_rva":begin,"end_rva":end,"span_bytes":body.len(),
                "span_sha256":expected_sha,"unwind_rva":unwind,
                "unwind_bytes_hex":codes.iter().map(|b|format!("{b:02x}")).collect::<String>(),
                "prologue_bytes_hex":prologue.iter().map(|b|format!("{b:02x}")).collect::<String>(),
                "claim_scope":"complete fixed disk function, not observed control flow/lifetime"}),
            )
        }
        fn rtti_failure(pe: &Pe<'_>) -> Result<Value, String> {
            let slot = 0xa06640;
            let raw = u64_at(pe.span(slot, 8, false)?, 0)?;
            if raw != 0xffff_ffff || raw.checked_sub(pe.base).is_some() {
                return Err("governor fixed non-COL predecessor differs".into());
            }
            Ok(
                json!({"table_rva":0xa06648,"preceding_slot_rva":slot,"raw_qword":raw,
                "status":"not_established_invalid_COL_pointer","source_type_name":null,
                "reason":"fixed predecessor is below preferred ImageBase; not a valid absolute x64 RTTI locator",
                "scope":"this table predecessor only; no claim about RTTI elsewhere"}),
            )
        }

        pub(super) fn inspect(core: &[u8]) -> Result<Value, String> {
            let pe = Pe::parse(core)?;
            let witnesses = spans(&pe)?;
            let refs = references(&pe)?;
            let functions = [
                complete(
                    &pe,
                    0x4a5f30,
                    0x4a63ad,
                    0xaf1928,
                    "0ae7315ceae95bf6def7024c984d9d5bd63df84886df36f494fbef1d321542c2",
                    &[
                        1, 0x1f, 0xc, 0, 0x1f, 0x74, 0x12, 0, 0x1f, 0x64, 0x11, 0, 0x1f, 0x34,
                        0x10, 0, 0x1f, 0x92, 0x18, 0xf0, 0x16, 0xe0, 0x14, 0xd0, 0x12, 0xc0, 0x10,
                        0x50,
                    ],
                )?,
                complete(
                    &pe,
                    0x4a64a0,
                    0x4a65b0,
                    0xacae3c,
                    "41fb6cefb7c5ab572672b84742aadf990669304009240219a5bafac49973d3f2",
                    &[1, 0xa, 4, 0, 0xa, 0x34, 6, 0, 0xa, 0x32, 6, 0x70],
                )?,
                complete(
                    &pe,
                    0x4a71a0,
                    0x4a71d4,
                    0xacae3c,
                    "0cb7eaae207fc94bb18c31525afbd42e3b24c07c75b7c17f034493f6c6be8506",
                    &[1, 0xa, 4, 0, 0xa, 0x34, 6, 0, 0xa, 0x32, 6, 0x70],
                )?,
                complete(
                    &pe,
                    0x4a71e0,
                    0x4a7214,
                    0xacae3c,
                    "8e77fe1a8e1ba3a58e2b1c3185bfd34991a0d0ac33769a366e3983f19e55594a",
                    &[1, 0xa, 4, 0, 0xa, 0x34, 6, 0, 0xa, 0x32, 6, 0x70],
                )?,
                complete(
                    &pe,
                    0x4a7380,
                    0x4a73c7,
                    0xb578e8,
                    "e23163f9d558e1676b6c93752235170e9df6f3a08c1f23deb41ad60b41a37a6c",
                    &[1, 4, 1, 0, 4, 0x62, 0, 0],
                )?,
            ];
            let slots = [
                pe.pointer(0xa06648, 0x4a71a0)?,
                pe.pointer(0xa06650, 0x4a8530)?,
                pe.pointer(0xa06790, 0x4a71e0)?,
            ];
            if pe.span(0x9c5fc0, 16, false)? != b"<Null Job Name>\0"
                || pe.span(0xa06920, 18, false)? != b"Governor Analyzer\0"
            {
                return Err("governor fixed name strings differ".into());
            }
            let fragment = pe
                .function(0x4a7a4b)
                .ok_or("governor producer pdata absent")?;
            let producer_chain = pe.chain(fragment)?;
            if producer_chain.last().is_none_or(|f| f.begin != 0x4a79b0) {
                return Err("governor producer chain owner differs".into());
            }
            Ok(
                json!({"schema":"todo880-governor-job-birth-owner-static-v1",
                "core_sha256":format!("{:x}",Sha256::digest(core)),"witnesses":witnesses,
                "complete_functions":functions,"computed_references":refs,"relocated_table_slots":slots,
                "rtti":rtti_failure(&pe)?,
                "owner_constructor":{"entry_rva":0x4a5f30,"input_receiver":"RCX saved into RDI at4A5F4F",
                    "normal_return":"RAX=RDI at4A6391, conditional on normal returns and Win64 nonvolatile ABI",
                    "table_rva":0xa06790,"allocation_caller_rva":0x4a7380,
                    "allocation_call_rva":0x4a73ac,"allocation_size_argument":0x1f0,
                    "constructor_tail_rva":0x4a73bd},
                "embedded_job":{"relation":"each checked function uses its own input owner+D0; cross-invocation receiver equality unestablished",
                    "offset":0xd0,"table_store_rva":0x4a60a1,"table_rva":0xa06648,
                    "sized_deallocation_argument":0x40,"argument_rva":0x4a71b9,
                    "construction_scope":"embedded initialization at this owner D0; no independent32-byte allocation in this slice",
                    "initial_fields":[{"offset":8,"value":0},{"offset":16,"value":0},
                        {"offset":24,"value":"Core+9C5FC0: <Null Job Name>"},
                        {"offset":32,"value":0},{"offset":40,"value":0},
                        {"offset":56,"width":2,"value":1}],
                    "native_ticket_field_write":false},
                "publication":{"producer_owner_entry":0x4a79b0,"job_address_rva":0x4a7a2e,
                    "job_address":"RDX=RSI+D0; RSI saved from incoming RCX",
                    "manager_load_rva":0x4a7a41,"manager_basis":"[Core+B8AA68]+250, not owner+250",
                    "manager_receiver":"RBX passed to both4A1100 and4A10B0 after normal Push return",
                    "metadata_rva":0xa06920,"metadata_text":"Governor Analyzer",
                    "producer_chain":producer_chain.iter().map(|f|json!({"begin":f.begin,"end":f.end,"unwind":f.unwind})).collect::<Vec<_>>()},
                "destruction":{"owner_table_virtual0":0x4a71e0,"owner_body":0x4a64a0,
                    "embedded_receiver_rva":0x4a6565,"embedded_receiver":"RCX=RBX+D0; RBX saved from owner RCX",
                    "base_reset_call_rva":0x4a656c,"base_reset_target":0x992150,"base_table_rva":0x9c5fd0,
                    "job_table_virtual0":0x4a71a0,"job_base_reset_call_rva":0x4a71af,
                    "distinct_paths":"owner body calls base reset directly, not the job virtual0",
                    "conditional_sized_call_target":0x9afea8,
                    "condition":"job/owner scalar paths test original DL bit0 after normal cleanup return",
                    "owner_size_argument":0x1f0,"job_size_argument":0x40},
                "certificate_issued":false,"runtime_calls":0,"game_runs":0,"observations_accepted":false,
                "zero_count_basis":"buffer-only implementation structure; not measured process counters",
                "not_established_by_this_audit":["executed constructor/producer/drop receiver equality",
                    "owner lifetime and its World epoch at construction","global manager identity with owner or its stored receiver",
                    "constructor/destruction ordering against pending or running proxy T",
                    "retain/copy/release/Cancel/reissue completion","safe birthticket transfer",
                    "4A8530 whole executed frame and SEH completion","986010 never-executed task recovery",
                    "all-domain Close/Rebuild authority"]}),
            )
        }
        #[cfg(test)]
        mod tests {
            use super::*;
            fn image() -> Vec<u8> {
                let mut b = vec![0u8; 0xb58000];
                for (rva, bytes) in [
                    (0x4a5f30, GOV_CTOR_PREFIX),
                    (0x4a606d, GOV_JOB_INIT),
                    (0x4a638c, GOV_CTOR_RETURN),
                    (0x4a64a0, GOV_OWNER_DROP),
                    (0x4a71a0, GOV_JOB_DROP),
                    (0x4a71e0, GOV_OWNER_DELETE),
                    (0x4a7380, GOV_CREATE),
                    (0x992150, GOV_JOB_BASE_RESET),
                ] {
                    let p = (rva - 0xc00) as usize;
                    b[p..p + bytes.len()].copy_from_slice(bytes);
                }
                let p = 0x4a600e - 0xc00;
                b[p..p + 14].copy_from_slice(&[
                    0x48, 0x8d, 0x05, 0x43, 6, 0x56, 0, 0x48, 0x8b, 0x55, 0xe0, 0x49, 0x89, 7,
                ]);
                let (_, block) = MANAGER_FLOW_BLOCKS
                    .iter()
                    .find(|(r, _)| *r == 0x4a7a27)
                    .unwrap();
                let p = 0x4a7a27 - 0xc00;
                b[p..p + block.len()].copy_from_slice(block);
                let p = 0xa06640 - 0x1400;
                b[p..p + 8].copy_from_slice(&0xffff_ffffu64.to_le_bytes());
                for (slot, target) in [
                    (0xa06648, 0x4a71a0),
                    (0xa06650, 0x4a8530),
                    (0xa06790, 0x4a71e0),
                ] {
                    let p = slot - 0x1400;
                    b[p..p + 8].copy_from_slice(&(0x180000000u64 + target).to_le_bytes());
                }
                b
            }
            fn pe(b: &[u8]) -> Pe<'_> {
                Pe {
                    image: b,
                    base: 0x180000000,
                    sections: vec![
                        Section {
                            name: *b".text\0\0\0",
                            rva: 0x1000,
                            virtual_size: 0x9b6000,
                            raw: 0x400,
                            size: 0x9b6000,
                            flags: 0x60000020,
                        },
                        Section {
                            name: *b".rdata\0\0",
                            rva: 0x9b7000,
                            virtual_size: 0x1a8124,
                            raw: 0x9b5c00,
                            size: 0x1a8200,
                            flags: 0x40000040,
                        },
                        Section {
                            name: *b".data\0\0\0",
                            rva: 0xb60000,
                            virtual_size: 0x30ff8,
                            raw: 0xb5de00,
                            size: 0x2a800,
                            flags: 0xc0000040,
                        },
                    ],
                    functions: vec![
                        Function {
                            begin: 0x4a5f30,
                            end: 0x4a63ad,
                            unwind: 0xaf1928,
                        },
                        Function {
                            begin: 0x4a64a0,
                            end: 0x4a65b0,
                            unwind: 0xacae3c,
                        },
                        Function {
                            begin: 0x4a71a0,
                            end: 0x4a71d4,
                            unwind: 0xacae3c,
                        },
                        Function {
                            begin: 0x4a71e0,
                            end: 0x4a7214,
                            unwind: 0xacae3c,
                        },
                        Function {
                            begin: 0x4a7380,
                            end: 0x4a73c7,
                            unwind: 0xb578e8,
                        },
                    ],
                    relocations: [0xa06648, 0xa06650, 0xa06790].into_iter().collect(),
                }
            }
            #[test]
            fn governor_birth_owner_offsets_prologues_and_destroy_receiver_mutations_reject() {
                let mut b = image();
                assert!(spans(&pe(&b)).is_ok());
                references(&pe(&b)).expect("valid governor reference fixture");
                for rva in [
                    0x4a5f50, 0x4a60a4, 0x4a607e, 0x4a6568, 0x4a71b7, 0x4a71ba, 0x4a7a31, 0x4a7a44,
                ] {
                    let p = rva - 0xc00;
                    b[p] ^= 1;
                    assert!(spans(&pe(&b)).is_err(), "mutation {rva:#x}");
                    b[p] ^= 1;
                }
                let mut wrong = pe(&b);
                wrong.functions[2].end += 1;
                assert!(spans(&wrong).is_err());
                assert!(spans(&pe(&b[..0x4a71d3 - 0xc00])).is_err());
            }
            #[test]
            fn governor_birth_computed_rel32_rip_targets_and_bounds_reject() {
                let mut b = image();
                references(&pe(&b)).expect("valid governor reference fixture");
                for rva in [0x4a73be, 0x4a71b0, 0x4a71f0, 0x4a656d, 0x4a609d, 0x4a7a2a] {
                    let p = rva - 0xc00;
                    b[p] ^= 1;
                    assert!(references(&pe(&b)).is_err());
                    b[p] ^= 1;
                }
                assert!(rel_target(0, &[0xe8, 0xf0, 0xff, 0xff, 0xff], 0xe8, 5, 1).is_err());
            }
            #[test]
            fn governor_singleton_zero_fill_is_an_address_not_disk_bytes() {
                let b = image();
                let mut mapped = pe(&b);
                references(&mapped).unwrap();
                assert!(mapped.span_any(0xb8aa68, 1).is_err());
                mapped.sections[2].virtual_size = 0x2aa68;
                assert!(references(&mapped).is_err());
                mapped.sections[2].virtual_size = 0x30ff8;
                mapped.sections[2].flags &= !0x8000_0000;
                assert!(references(&mapped).is_err());
                mapped.sections[2].flags = 0xc0000040;
                mapped.sections.push(mapped.sections[2].clone());
                assert!(references(&mapped).is_err());
            }
            #[test]
            fn governor_birth_rtti_absence_is_fixed_failure_not_a_type_or_adjacent_table_claim() {
                let mut b = image();
                let v = rtti_failure(&pe(&b)).unwrap();
                assert!(v["source_type_name"].is_null());
                assert_eq!(v["status"], "not_established_invalid_COL_pointer");
                let p = 0xa06640 - 0x1400;
                b[p..p + 8].copy_from_slice(&0x180a06600u64.to_le_bytes());
                assert!(rtti_failure(&pe(&b)).is_err());
                let mut table = pe(&b);
                assert!(table.pointer(0xa06648, 0x4a71a0).is_ok());
                table.relocations.remove(&0xa06648);
                assert!(table.pointer(0xa06648, 0x4a71a0).is_err());
                assert!(pe(&b).pointer(0xa06648, 0x4a71e0).is_err());
            }
            #[test]
            fn governor_birth_full_span_sha_unwind_and_pdata_are_independent_guards() {
                let mut b = image();
                let p = 0xacae3c - 0x1400;
                let codes = [1, 0xa, 4, 0, 0xa, 0x34, 6, 0, 0xa, 0x32, 6, 0x70];
                b[p..p + codes.len()].copy_from_slice(&codes);
                let expected = "41fb6cefb7c5ab572672b84742aadf990669304009240219a5bafac49973d3f2";
                assert!(complete(&pe(&b), 0x4a64a0, 0x4a65b0, 0xacae3c, expected, &codes).is_ok());
                b[0x4a6508 - 0xc00] ^= 1;
                assert!(complete(&pe(&b), 0x4a64a0, 0x4a65b0, 0xacae3c, expected, &codes).is_err());
                b[0x4a6508 - 0xc00] ^= 1;
                b[p + 5] ^= 1;
                assert!(complete(&pe(&b), 0x4a64a0, 0x4a65b0, 0xacae3c, expected, &codes).is_err());
            }
        }
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
    require_image_pair(&core_sha, &host_sha)?;
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
        (0x986450, 0xe9, 0x985970),
        (0x986460, 0xe9, 0x985a80),
        (0x986470, 0xe9, 0x985bd0),
        (0x986492, 0xe8, 0x985f40),
        (0x9864a0, 0xe9, 0x986010),
        (0x98626b, 0xe8, 0xe3eaf0),
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
    bytes(host, 0x986270, &[0x48, 0xc7, 0x47, 8, 0, 0, 0, 0])?;
    bytes(host, 0x986232, &[0x48, 0x8b, 7, 0x48, 0x89, 0x47, 8])?;
    Ok(json!({
        "schema":"todo880-native-job-lifecycle-static-v7", "image_revision":IMAGE_REVISION,
        "core_sha256":core_sha,"host_sha256":host_sha,"witnesses":witnesses,
        "manager":{"from_world_offset":0x250,"job_set_offset":0x400,"list_id_offset":0x410,"lifecycle_state_offset":0x38a8},
        "abi":{
            "push":{"rcx":"list ID (native host uses low32, IDs1..64)","rdx":"native job object", "r8":"manager+400 job-set address", "r9":"pointer to 16-byte metadata copy"},
            "wait":{"rcx":"same job-set address", "rdx":"pointer to 16-byte metadata copy", "result":"normal return, not RAX=true; host clears set+8 after native arena wait"}
        },
        "receipt_candidates":{"wait_return_core":0x4a1196,"conditional_host_callback_merge_core":0x4a11b3,"free_list_return_core":0x4a120e,"host_wait_pointer_clear":0x986270},
        "file_offset_basis":"SHA-pinned images with independently verified text RVA1000/raw400; Core rdata RVA9B7000/raw9B5C00 virtual size1A8124 and preferred ImageBase180000000; not a general PE parser",
        "release_leaf":{"wrapper_rva":0x58ff70,"leaf_rva":0x7bbf0,"instruction_bytes":14,"same_byte_target_rva":read_target,"behavior":"non-atomic read then write of one RIP-relative byte; not world teardown","world_shutdown_link_evidence":"not established by this audit"},
        "byte_b61e00_consumer":byte_b61e00_consumer(core, read_target)?,
        "manager_receiver_flow":manager_receiver_flow(core)?,
        "governor_job_birth_owner":worker_payload::inspect_governor_birth(core)?,
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
    fn versioned_image_pair_requires_both_exact_new_files() {
        let old_core = "324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c";
        let old_host = "e7450823cc8e00468cff7b9d7b97c63140eae38ae1d774ba4efa437556c42d63";
        require_image_pair(CORE_SHA, HOST_SHA).unwrap();
        for (core, host) in [
            (old_core, old_host),
            (old_core, HOST_SHA),
            (CORE_SHA, old_host),
            ("", HOST_SHA),
            (CORE_SHA, ""),
            (&CORE_SHA[..63], HOST_SHA),
        ] {
            assert!(require_image_pair(core, host).is_err());
        }
        let mut changed = CORE_SHA.to_owned();
        changed.replace_range(0..1, "1");
        assert!(require_image_pair(&changed, HOST_SHA).is_err());
        assert!(require_image_pair(CORE_SHA, &HOST_SHA.to_ascii_uppercase()).is_err());
    }

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
