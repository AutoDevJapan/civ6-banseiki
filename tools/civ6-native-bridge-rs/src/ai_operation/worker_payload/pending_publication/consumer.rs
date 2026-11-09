//! Fixed local consumer flow. No scheduling, cancellation or hook activation.
use super::*;

fn blocks(pe: &Pe<'_>) -> Result<Vec<Value>, String> {
    BLOCKS
        .iter()
        .map(|&(r, b)| pe.witness(r, b, false))
        .collect()
}

fn references(pe: &Pe<'_>) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for &(r, prefix, length, offset, target, kind) in BRANCHES {
        let code = pe.span(r, length, true)?;
        if !code.starts_with(prefix) || relative_target(r, code, length, offset)? != target {
            return Err(format!("consumer {kind} operand differs at {r:#x}"));
        }
        pe.span(target, 1, true)?;
        out.push(json!({"rva":r,"kind":kind,"computed_target_rva":target}));
    }
    // The existing pending-publication audit checks the first two slots.
    // This third slot is the affinity notification reached after tag unwrapping.
    let slot = 0x106c428;
    let target = 0x967b0;
    let raw = u64_at(pe.span(slot, 8, false)?, 0)?;
    if raw.checked_sub(pe.base) != Some(u64::from(target)) {
        return Err("consumer preferred-image affinity slot differs".into());
    }
    pe.witness(target, &[0xc2, 0, 0], false)?;
    out.push(json!({"slot_rva":slot,"computed_target_rva":target,
        "scope":"preferred-image disk pointer only; not an executed table identity"}));
    Ok(out)
}

fn relative_target(rva: u32, code: &[u8], length: usize, offset: usize) -> Result<u32, String> {
    if length == 2 {
        if code.len() != 2 || offset != 1 {
            return Err("consumer short branch length/operand".into());
        }
        u32::try_from(i64::from(rva) + 2 + i64::from(code[1] as i8))
            .map_err(|_| "consumer short branch target bounds".into())
    } else {
        rel_target(rva, code, code[0], length, offset)
    }
}

pub(super) fn inspect(pe: &Pe<'_>) -> Result<Value, String> {
    let frames = FRAMES
        .iter()
        .map(|s| frame(pe, s))
        .collect::<Result<Vec<_>, _>>()?;
    let blocks = blocks(pe)?;
    let refs = references(pe)?;
    Ok(json!({"schema":"native-queue-consumer-local-static-v1",
        "host_sha256":HOST_SHA,"frames":frames,"contiguous_blocks":blocks,"references":refs,
        "typed_boundary":{
            "input":"E417F6 and E41C31 save entry R8 as RBX; that raw input is not intrinsically a typed T",
            "known_T":"conditional on the retained birth reservation identifying this receiver as the original 16-byte Host proxy T with table106C418; T+8 is P, not a generation ticket",
            "tag_proxy":"different scheduler object; [proxy-B]==20 selects its +8 tagged-pointer CAS path",
            "affinity_slot":"the known T table+10 is0967B0 (RET0); calling this slot does not change the T identity under the verified leaf body",
            "assumption":"normal-return callees preserve Win64 nonvolatile registers and caller stack; runtime receiver/table continuity is not certified"},
        "consumer_loops":[
            {"entry":0xe417d0,"context_load":0xe418a6,"context":"[RBX-38]",
             "pre_execute_state_store":0xe418ae,"skip_execute_branch":0xe418b6,
             "skip_if":"[context+78]!=0; branch target E418F5",
             "execute_call":0xe418d0,"execute_receiver":"RCX=RBX; RAX=[RBX]; call[RAX+8]",
             "state_zero_branch":0xe418fb,"destroy_call":0xe419c6,
             "destroy_receiver":"RCX=the same RBX; EDX=0; call[[RBX]]; this also occurs on the local execute-skip path"},
            {"entry":0xe41c00,"context_load":0xe41cb7,"context":"[RBX-38]",
             "pre_execute_state_store":0xe41cbf,"skip_execute_branch":0xe41cc7,
             "skip_if":"[context+78]!=0; branch target E41CDE",
             "execute_call":0xe41ccf,"execute_receiver":"RCX=RBX; RAX=[RBX]; call[RAX+8]",
             "state_zero_branch":0xe41ce4,"destroy_call":0xe41d95,
             "destroy_receiver":"RCX=the same RBX; EDX=0; call[[RBX]]; this also occurs on the local execute-skip path"}],
        "local_array_handoff":{
            "callers":[0xe41aa3,0xe41e60],"callee":0xe41140,
            "load_sites":[0xe411b2,0xe4122b],"return_site":0xe4126b,
            "flow":"slot qword -> RDI; ordinary T bypasses tag-proxy path; RAX=RDI; callers save RAX in RBX before looping to virtual execute/destructor",
            "limits":"local pointer flow only; array exclusivity and every other scheduler dispatch table are not established"},
        "tag_claims":[
            {"load":0xe41090,"cas":0xe4109d,"clear_tag":0xe410a5,"replacement":1,
             "flow":"[proxy+8] -> RBX/RAX; CMPXCHG success -> RBX&~3; nonzero result returns original T"},
            {"load":0xe41241,"cas":0xe4124d,"clear_tag":0xe41254,"replacement":2,
             "flow":"[proxy+8] -> RDI/RAX; CMPXCHG success -> RDI&~3; nonzero result passes affinity slot+10 and returns original T"},
            {"load":0xe425ce,"cas":0xe425ec,"clear_tag":0xe425f7,"replacement":2},
            {"load":0xe4294d,"cas":0xe4296b,"clear_tag":0xe42972,"replacement":2}],
        "unexecuted_T_terminal":{
            "local_fact":"execute-skip leaves the explicitly written state0; the checked conditional branch reaches slot0 with the same T and EDX0",
            "known_target":0x9857c0,
            "destructor_scope":"normal non-delete destructor return settles only that T invocation; its body never dereferences/releases P",
            "one_claim_scope":"CAS arbitrates one shared proxy+8 slot; no global exactly-once claim over all copies, queues or reissues"},
        "birth_frontier":{
            "caller":0x985f99,"callee":0xe3ccf0,"normal_return":0x985f9e,
            "allocation_result":"E3CDA4 restores RAX from RBP saved atE3CD44 after allocator callE3CD3A; normal RET atE3CDBF",
            "before_T_initialization":"caller first sets R8=RAX at985F9E, checks nonzero, stores vft at985FB1 and P at985FB4",
            "before_pending_publication":"same normal path later stores [P+8]=T at985FE3 and list-tail links at985FFD/986000",
            "gateway_candidate":{"entry":0xe3ccf0,"verify_first_8_hex":"48895c240848896c",
                "stolen_instruction_lengths":[5],"continuation":0xe3ccf5,
                "first_instruction":"mov [rsp+8],rbx; next instruction startsE3CCF5",
                "scope":"whole-function gateway may bind opaque normal RAX to an ancestor Push reservation before returning to985F9E; not a mid-caller patch or proof that any particular gateway is installed"},
            "adapter_requirements":["select exact return985F9E and the same-stack outstanding Push reservation; other allocator callers remain transparent",
                "use P/set/generation from the ancestor reservation; do not read uninitialized T or write a private ticket into P",
                "preserve all original return state, nonvolatile registers, flags, LastError, stack alignment and unwind ownership",
                "invoke the original exactly once; SEH propagates the original unwind and never binds a normal result",
                "stolen save-RBX instruction needs matching unwind at its real stack offset; complete allocator unwind remains original after continuation"]},
        "not_established_by_this_audit":[
            "all producer admission or natural seal; state3, FreeList and set0 are not seals",
            "runtime T/P/set/world-generation identity or lifetime",
            "global T exactly-once, every copied/tagged proxy, queue exclusivity and all cancellation paths",
            "typed scheduler virtual+30 target atE41AE2/E41E9F and all alternate queue-to-consumer paths",
            "same-set full join, P-owner destruction permission or all-domain Close/Rebuild",
            "operation synchronous dispatcher domains behind Core tables9BE250/9BE370/9BEC58"],
        "certificate_issued":false,
        "structural_declarations":{"runtime_calls":0,"game_runs":0,
            "basis":"file-buffer-only audit structure, not measured native execution counters"}}))
}

const FRAMES: &[Frame] = &[
    Frame {
        begin: 0xe3ccf0,
        end: 0xe3cdc0,
        sha: "1fb54c0082a8474969d3dcf628d293d023af375ff7e2614b573974c530de9e76",
        fragments: &[(
            0xe3ccf0,
            0xe3cdc0,
            0x11c12d4,
            "0114080014640800145407001434060014321070",
        )],
    },
    Frame {
        begin: 0xe41060,
        end: 0xe41136,
        sha: "afaf5f31d5eb064ef285ac4a002d1cdca9c8f0068428a74ad7d27dfed93d3328",
        fragments: &[(
            0xe41060,
            0xe41136,
            0x118038c,
            "010f06000f6408000f3407000f320b70",
        )],
    },
    Frame {
        begin: 0xe41140,
        end: 0xe412d8,
        sha: "1a0ba793cd89866d27217dcc184b888de963591f118eae31123542a50e012a7b",
        fragments: &[(
            0xe41140,
            0xe412d8,
            0x123f83c,
            "0112080012540a001234090012320ee00c700b60",
        )],
    },
    Frame {
        begin: 0xe412e0,
        end: 0xe41396,
        sha: "2b0a6dbde3b316e793a3e17ec77cf09314390cb82fb7ecf60fa5cab185d0d98e",
        fragments: &[
            (0xe412e0, 0xe41302, 0x11cdca4, "010a04000a5409000a320660"),
            (
                0xe41302,
                0xe41326,
                0x123a620,
                "2105020005740800e012e4000213e400a4dc1c01",
            ),
            (
                0xe41326,
                0xe4133d,
                0x123a634,
                "2100020000740800e012e4000213e400a4dc1c01",
            ),
            (
                0xe4133d,
                0xe41383,
                0x123a648,
                "21050200053407002613e4003d13e40034a62301",
            ),
            (
                0xe41383,
                0xe41396,
                0x123a65c,
                "210000002613e4003d13e40034a62301",
            ),
        ],
    },
    Frame {
        begin: 0xe417d0,
        end: 0xe41bf1,
        sha: "9cfb21b3ac5a6673fd3de84483fd5bc7a250f58c13a02eee87c0e9c802a8d36f",
        fragments: &[(
            0xe417d0,
            0xe41bf1,
            0x123a8f8,
            "011f0c001f7415001f6414001f3413001fb218f016e014d012c01050",
        )],
    },
    Frame {
        begin: 0xe41c00,
        end: 0xe41fae,
        sha: "8a383bb62e2a13f1e78fba6e3e06139fb35330a2ae984736944639ae8a527251",
        fragments: &[(
            0xe41c00,
            0xe41fae,
            0x123a8f8,
            "011f0c001f7415001f6414001f3413001fb218f016e014d012c01050",
        )],
    },
    Frame {
        begin: 0xe42470,
        end: 0xe427e5,
        sha: "5300ce31e68659e0dca1b58453d1707f434c4af177aee72565b2f0a32426949d",
        fragments: &[(
            0xe42470,
            0xe427e5,
            0x123a8e0,
            "01140a0014341000145210f00ee00cd00ac0087007600650",
        )],
    },
    Frame {
        begin: 0xe427f0,
        end: 0xe42ad7,
        sha: "ce51064e6ac7157700df4da6021441cf709b2329402df8186de4753667b884de",
        fragments: &[(
            0xe427f0,
            0xe42ad7,
            0x123a8e0,
            "01140a0014341000145210f00ee00cd00ac0087007600650",
        )],
    },
    Frame {
        begin: 0xe41fb0,
        end: 0xe42075,
        sha: "9dfe6d404f315234a7aa08bdeae3c4a5b440b4c96ff459e5478ae61c96bd39ce",
        fragments: &[
            (
                0xe41fb0,
                0xe41fd4,
                0x11c7038,
                "010c06000c340a000c32087007600650",
            ),
            (
                0xe41fd4,
                0xe4205c,
                0x123a66c,
                "2105020005e40800b01fe400d41fe40038701c01",
            ),
            (
                0xe4205c,
                0xe42065,
                0x123a680,
                "2100020000e40800b01fe400d41fe40038701c01",
            ),
            (
                0xe42065,
                0xe42075,
                0x123a694,
                "21000000b01fe400d41fe40038701c01",
            ),
        ],
    },
    Frame {
        begin: 0xe42b80,
        end: 0xe42cd0,
        sha: "84e564791fa187eb3a049cdde7b6babad5e519d7c5a1fd1816e7a4c5f3294fa4",
        fragments: &[
            (0xe42b80, 0xe42ba7, 0x123a6f8, "010a04000a5206e004d00270"),
            (
                0xe42ba7,
                0xe42c53,
                0x123a704,
                "211f0a001ff4040016c4050011640c000a540b0005340a00802be400a72be400f8a62301",
            ),
            (
                0xe42c53,
                0xe42c68,
                0x123a728,
                "2100040000640c0000540b00802be400a72be400f8a62301",
            ),
            (
                0xe42c68,
                0xe42c86,
                0x123a740,
                "2100020000640c00802be400a72be400f8a62301",
            ),
            (
                0xe42c86,
                0xe42cd0,
                0x123a754,
                "21000a0000f4040000c4050000640c0000540b0000340a00802be400a72be400f8a62301",
            ),
        ],
    },
];

const BLOCKS: &[(u32, &[u8])] = &[
    (
        0xe3ccf0,
        &[
            0x48, 0x89, 0x5c, 0x24, 0x08, 0x48, 0x89, 0x6c, 0x24, 0x10, 0x48, 0x89, 0x74, 0x24,
            0x18, 0x57, 0x48, 0x83, 0xec, 0x20,
        ],
    ),
    (
        0xe3cd2e,
        &[
            0x4c, 0x8b, 0x0f, 0x45, 0x33, 0xc0, 0x48, 0x8b, 0xd6, 0x48, 0x8b, 0xcb, 0xe8, 0x91,
            0x3b, 0x00, 0x00, 0x48, 0x8b, 0x0f, 0x33, 0xf6, 0x48, 0x8b, 0xe8,
        ],
    ),
    (
        0xe3cda1,
        &[
            0x48, 0x8b, 0x0f, 0x48, 0x8b, 0xc5, 0x48, 0x8b, 0x5c, 0x24, 0x30, 0x48, 0x8b, 0x6c,
            0x24, 0x38, 0x48, 0x89, 0x71, 0x20, 0x48, 0x8b, 0x74, 0x24, 0x40, 0x48, 0x83, 0xc4,
            0x20, 0x5f, 0xc3,
        ],
    ),
    (
        0x985f86,
        &[
            0x48, 0x8b, 0x43, 0x08, 0x48, 0x8d, 0x4c, 0x24, 0x40, 0xba, 0x10, 0x00, 0x00, 0x00,
            0x48, 0x89, 0x44, 0x24, 0x40, 0xe8, 0x52, 0x6d, 0x4b, 0x00, 0x4c, 0x8b, 0xc0, 0x48,
            0x85, 0xc0, 0x74, 0x14, 0xc6, 0x40, 0xf5, 0x01, 0x48, 0x8d, 0x05, 0x67, 0x64, 0x6e,
            0x00, 0x49, 0x89, 0x00, 0x49, 0x89, 0x78, 0x08, 0xeb, 0x03, 0x45, 0x33, 0xc0,
        ],
    ),
    (
        0x985fbd,
        &[
            0x0f, 0x28, 0x44, 0x24, 0x20, 0x8d, 0x55, 0xff, 0x48, 0x8b, 0x5c, 0x24, 0x48, 0x48,
            0x8d, 0x05, 0xff, 0x42, 0x38, 0x02, 0x48, 0x8b, 0x6c, 0x24, 0x50, 0x49, 0x8d, 0x48,
            0xf8, 0x48, 0x89, 0x77, 0x10, 0x48, 0x8b, 0x74, 0x24, 0x58, 0x4c, 0x89, 0x47, 0x08,
            0x0f, 0x11, 0x47, 0x18, 0x48, 0xc7, 0x01, 0x00, 0x00, 0x00, 0x00, 0x48, 0xc1, 0xe2,
            0x05, 0x48, 0x03, 0xd0, 0x48, 0x8b, 0x42, 0x08, 0x4c, 0x89, 0x00, 0x48, 0x89, 0x4a,
            0x08, 0x48, 0x83, 0xc4, 0x30, 0x5f, 0xc3,
        ],
    ),
    (
        0xe417ef,
        &[
            0x48, 0x8b, 0x05, 0xea, 0x37, 0x45, 0x00, 0x49, 0x8b, 0xd8, 0x4c, 0x8b, 0xea, 0x4c,
            0x8b, 0xf1,
        ],
    ),
    (
        0xe41c1f,
        &[
            0x48, 0x8b, 0x41, 0x30, 0x48, 0x8b, 0xf1, 0x48, 0x89, 0x45, 0x30, 0x4c, 0x8b, 0xea,
            0x48, 0x8b, 0x41, 0x28, 0x49, 0x8b, 0xd8, 0x48, 0x89, 0x41, 0x30,
        ],
    ),
    (
        0xe418a0,
        &[
            0x49, 0x89, 0x5e, 0x28, 0x33, 0xf6, 0x48, 0x8b, 0x4b, 0xc8, 0x4c, 0x89, 0x73, 0xd8,
            0x40, 0x88, 0x73, 0xf4, 0x48, 0x39, 0x71, 0x78, 0x75, 0x3d, 0x48, 0x8b, 0x05, 0x61,
            0x53, 0x45, 0x00, 0x48, 0x85, 0xc0, 0x74, 0x06, 0x48, 0x8b, 0x49, 0x20, 0xff, 0xd0,
            0x48, 0x8b, 0x03, 0x48, 0x8b, 0xcb, 0xff, 0x50, 0x08, 0x48, 0x8b, 0xf0,
        ],
    ),
    (
        0xe418f5,
        &[
            0x0f, 0xb6, 0x4b, 0xf4, 0x85, 0xc9, 0x0f, 0x84, 0xb9, 0x00, 0x00, 0x00,
        ],
    ),
    (
        0xe419ba,
        &[
            0x48, 0x8b, 0x03, 0x33, 0xd2, 0x48, 0x8b, 0x7b, 0xe0, 0x48, 0x8b, 0xcb, 0xff, 0x10,
        ],
    ),
    (
        0xe41cb0,
        &[
            0x48, 0x89, 0x5e, 0x28, 0x45, 0x33, 0xf6, 0x48, 0x8b, 0x43, 0xc8, 0x48, 0x89, 0x73,
            0xd8, 0x44, 0x88, 0x73, 0xf4, 0x4c, 0x39, 0x70, 0x78, 0x75, 0x15, 0x48, 0x8b, 0x03,
            0x48, 0x8b, 0xcb, 0xff, 0x50, 0x08, 0x4c, 0x8b, 0xf0,
        ],
    ),
    (
        0xe41cde,
        &[
            0x0f, 0xb6, 0x53, 0xf4, 0x85, 0xd2, 0x0f, 0x84, 0x9f, 0x00, 0x00, 0x00,
        ],
    ),
    (
        0xe41d89,
        &[
            0x48, 0x8b, 0x03, 0x33, 0xd2, 0x48, 0x8b, 0x7b, 0xe0, 0x48, 0x8b, 0xcb, 0xff, 0x10,
        ],
    ),
    (
        0xe41aa0,
        &[
            0x49, 0x8b, 0xce, 0xe8, 0x98, 0xf6, 0xff, 0xff, 0x48, 0x8b, 0xd8, 0x48, 0x85, 0xc0,
            0x74, 0x19, 0x48, 0x8b, 0x48, 0xc8, 0x8b, 0x45, 0xe8, 0x39, 0x41, 0x70, 0x0f, 0x85,
            0xc4, 0xfd, 0xff, 0xff, 0x0f, 0xb7, 0x45, 0xec, 0xe9, 0xb5, 0xfd, 0xff, 0xff,
        ],
    ),
    (
        0xe41e5d,
        &[
            0x48, 0x8b, 0xce, 0xe8, 0xdb, 0xf2, 0xff, 0xff, 0x48, 0x8b, 0xd8, 0x48, 0x85, 0xc0,
            0x74, 0x19, 0x48, 0x8b, 0x48, 0xc8, 0x8b, 0x45, 0xe8, 0x39, 0x41, 0x70, 0x0f, 0x85,
            0x11, 0xfe, 0xff, 0xff, 0x0f, 0xb7, 0x45, 0xec, 0xe9, 0x02, 0xfe, 0xff, 0xff,
        ],
    ),
    (
        0xe411a7,
        &[
            0x48, 0x8b, 0x43, 0x18, 0x48, 0x8b, 0x88, 0x98, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x3c,
            0xf1, 0x7d, 0x28, 0x48, 0x8b, 0xc8, 0x48, 0x39, 0x68, 0x08, 0x74, 0x6e,
        ],
    ),
    (
        0xe41220,
        &[
            0x48, 0x8b, 0x43, 0x18, 0x48, 0x8b, 0x88, 0x98, 0x00, 0x00, 0x00, 0x48, 0x8b, 0x3c,
            0xf1, 0x48, 0x85, 0xff, 0x74, 0x37, 0x80, 0x7f, 0xf5, 0x20, 0x75, 0x31, 0x48, 0x8d,
            0x57, 0x08, 0x48, 0x8b, 0xcf, 0x48, 0x8b, 0x3a, 0x48, 0x83, 0xff, 0x01, 0x74, 0x37,
            0x48, 0x8b, 0xc7, 0xf0, 0x4c, 0x0f, 0xb1, 0x32, 0x75, 0x2d, 0x48, 0x83, 0xe7, 0xfc,
            0x74, 0x2a, 0x0f, 0xb7, 0x53, 0x40, 0x48, 0x8b, 0xcf, 0x48, 0x89, 0x7b, 0x28, 0x48,
            0x8b, 0x07, 0xff, 0x50, 0x10, 0x48, 0x8b, 0xc7, 0x48, 0x8b, 0x5c, 0x24, 0x48, 0x48,
            0x8b, 0x6c, 0x24, 0x50, 0x48, 0x83, 0xc4, 0x20, 0x41, 0x5e, 0x5f, 0x5e, 0xc3, 0x48,
            0x8b, 0xfd,
        ],
    ),
    (
        0xe41140,
        &[
            0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x6c, 0x24, 0x18, 0x56, 0x57, 0x41, 0x56,
            0x48, 0x83, 0xec, 0x20, 0x48, 0x8b, 0x41, 0x18, 0x33, 0xed, 0x48, 0x8b, 0xd9, 0x8b,
            0xfd, 0x48, 0x8b, 0xb0, 0x88, 0x00, 0x00, 0x00, 0x44, 0x8d, 0x75, 0x02,
        ],
    ),
    (
        0xe41060,
        &[
            0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x74, 0x24, 0x18, 0x57, 0x48, 0x83, 0xec,
            0x20, 0x48, 0x8b, 0xf9, 0x48, 0x8b, 0x49, 0x38, 0xe8, 0x65, 0x02, 0x00, 0x00, 0x48,
            0x8b, 0xd0, 0x48, 0x85, 0xc0, 0x74, 0x78, 0xbe, 0x01, 0x00, 0x00, 0x00,
        ],
    ),
    (
        0xe41090,
        &[
            0x48, 0x8b, 0x5a, 0x08, 0x48, 0x83, 0xfb, 0x02, 0x74, 0x11, 0x48, 0x8b, 0xc3, 0xf0,
            0x48, 0x0f, 0xb1, 0x72, 0x08, 0x75, 0x06, 0x48, 0x83, 0xe3, 0xfc, 0x75, 0x62,
        ],
    ),
    (
        0xe410ea,
        &[
            0x48, 0x8b, 0x4f, 0x38, 0xe8, 0xed, 0x01, 0x00, 0x00, 0x48, 0x8b, 0xd0, 0x48, 0x85,
            0xc0, 0x75, 0x95, 0x33, 0xc0, 0x48, 0x8b, 0x5c, 0x24, 0x38, 0x48, 0x8b, 0x74, 0x24,
            0x40, 0x48, 0x83, 0xc4, 0x20, 0x5f, 0xc3,
        ],
    ),
    (
        0xe4111f,
        &[
            0x80, 0x4b, 0xf5, 0x80, 0x48, 0x8b, 0xc3, 0x48, 0x8b, 0x5c, 0x24, 0x38, 0x48, 0x8b,
            0x74, 0x24, 0x40, 0x48, 0x83, 0xc4, 0x20, 0x5f, 0xc3,
        ],
    ),
    (
        0xe425b7,
        &[
            0xe8, 0xc4, 0x05, 0x00, 0x00, 0x48, 0x8b, 0xf8, 0x48, 0x85, 0xc0, 0x0f, 0x84, 0xd4,
            0x00, 0x00, 0x00, 0x80, 0x78, 0xf5, 0x20, 0x75, 0x33, 0x48, 0x8b, 0x78, 0x08, 0x48,
            0x8d, 0x48, 0x08, 0x48, 0x8b, 0xd0, 0x48, 0x83, 0xff, 0x01, 0x0f, 0x84, 0xaf, 0x00,
            0x00, 0x00, 0x48, 0x8b, 0xc7, 0x41, 0xb8, 0x02, 0x00, 0x00, 0x00, 0xf0, 0x4c, 0x0f,
            0xb1, 0x01, 0x0f, 0x85, 0x9b, 0x00, 0x00, 0x00, 0x48, 0x83, 0xe7, 0xfc, 0x0f, 0x84,
            0x93, 0x00, 0x00, 0x00, 0x80, 0x4f, 0xf5, 0x80, 0x0f, 0xb6, 0x47, 0xf5, 0x24, 0x0f,
            0x3c, 0x01, 0x72, 0x15, 0x48, 0x89, 0x7b, 0x28, 0x48, 0x8b, 0xcf, 0x48, 0x8b, 0x07,
            0x48, 0x89, 0x5f, 0xd8, 0x0f, 0xb7, 0x53, 0x40, 0xff, 0x50, 0x10,
        ],
    ),
    (
        0xe42692,
        &[0x33, 0xff, 0x48, 0x8b, 0xcb, 0xe8, 0x54, 0xde, 0xff, 0xff],
    ),
    (
        0xe42936,
        &[
            0xe8, 0x45, 0x02, 0x00, 0x00, 0x48, 0x8b, 0xd8, 0x48, 0x85, 0xc0, 0x0f, 0x84, 0xb2,
            0x00, 0x00, 0x00, 0x80, 0x78, 0xf5, 0x20, 0x75, 0x2b, 0x48, 0x8b, 0x58, 0x08, 0x48,
            0x8d, 0x48, 0x08, 0x48, 0x8b, 0xd0, 0x48, 0x83, 0xfb, 0x01, 0x0f, 0x84, 0x8d, 0x00,
            0x00, 0x00, 0x48, 0x8b, 0xc3, 0x41, 0xb8, 0x02, 0x00, 0x00, 0x00, 0xf0, 0x4c, 0x0f,
            0xb1, 0x01, 0x75, 0x7d, 0x48, 0x83, 0xe3, 0xfc, 0x74, 0x79, 0x80, 0x4b, 0xf5, 0x80,
            0x0f, 0xb6, 0x43, 0xf5, 0x24, 0x0f, 0x3c, 0x01, 0x72, 0x15, 0x48, 0x89, 0x5f, 0x28,
            0x48, 0x8b, 0xcb, 0x48, 0x8b, 0x03, 0x48, 0x89, 0x7b, 0xd8, 0x0f, 0xb7, 0x57, 0x40,
            0xff, 0x50, 0x10,
        ],
    ),
    (
        0xe429ef,
        &[0x33, 0xdb, 0x48, 0x8b, 0xcf, 0xe8, 0xf7, 0xda, 0xff, 0xff],
    ),
    (
        0xe41ad8,
        &[
            0x49, 0x8b, 0x06, 0x49, 0x8d, 0x55, 0xe8, 0x49, 0x8b, 0xce, 0xff, 0x50, 0x30, 0x48,
            0x8b, 0xd8, 0x48, 0x85, 0xc0, 0x74, 0x5d, 0xeb, 0xc1,
        ],
    ),
    (
        0xe41e95,
        &[
            0x48, 0x8b, 0x06, 0x49, 0x8d, 0x55, 0xe8, 0x48, 0x8b, 0xce, 0xff, 0x50, 0x30, 0x48,
            0x8b, 0xd8, 0x48, 0x85, 0xc0, 0x74, 0x5d, 0xeb, 0xc1,
        ],
    ),
    (
        0xe4126b,
        &[
            0x48, 0x8b, 0xc7, 0x48, 0x8b, 0x5c, 0x24, 0x48, 0x48, 0x8b, 0x6c, 0x24, 0x50, 0x48,
            0x83, 0xc4, 0x20, 0x41, 0x5e, 0x5f, 0x5e, 0xc3,
        ],
    ),
];

type RelativeEdge = (u32, &'static [u8], usize, usize, u32, &'static str);
const BRANCHES: &[RelativeEdge] = &[
    (0x985f99, &[0xe8], 5, 1, 0xe3ccf0, "direct_call"),
    (0x985fa4, &[0x74], 2, 1, 0x985fba, "short_jump"),
    (0x985fb8, &[0xeb], 2, 1, 0x985fbd, "short_jump"),
    (0xe3cd3a, &[0xe8], 5, 1, 0xe408d0, "direct_call"),
    (0xe418b6, &[0x75], 2, 1, 0xe418f5, "short_jump"),
    (0xe418fb, &[0x0f, 0x84], 6, 2, 0xe419ba, "conditional_rel32"),
    (0xe41cc7, &[0x75], 2, 1, 0xe41cde, "short_jump"),
    (0xe41ce4, &[0x0f, 0x84], 6, 2, 0xe41d89, "conditional_rel32"),
    (0xe41aa3, &[0xe8], 5, 1, 0xe41140, "direct_call"),
    (0xe41aae, &[0x74], 2, 1, 0xe41ac9, "short_jump"),
    (0xe41aba, &[0x0f, 0x85], 6, 2, 0xe41884, "conditional_rel32"),
    (0xe41ac4, &[0xe9], 5, 1, 0xe4187e, "direct_jump"),
    (0xe41e60, &[0xe8], 5, 1, 0xe41140, "direct_call"),
    (0xe41e6b, &[0x74], 2, 1, 0xe41e86, "short_jump"),
    (0xe41e77, &[0x0f, 0x85], 6, 2, 0xe41c8e, "conditional_rel32"),
    (0xe41e81, &[0xe9], 5, 1, 0xe41c88, "direct_jump"),
    (0xe41232, &[0x74], 2, 1, 0xe4126b, "short_jump"),
    (0xe41238, &[0x75], 2, 1, 0xe4126b, "short_jump"),
    (0xe41248, &[0x74], 2, 1, 0xe41281, "short_jump"),
    (0xe41252, &[0x75], 2, 1, 0xe41281, "short_jump"),
    (0xe41258, &[0x74], 2, 1, 0xe41284, "short_jump"),
    (0xe41076, &[0xe8], 5, 1, 0xe412e0, "direct_call"),
    (0xe41081, &[0x74], 2, 1, 0xe410fb, "short_jump"),
    (0xe41098, &[0x74], 2, 1, 0xe410ab, "short_jump"),
    (0xe410a3, &[0x75], 2, 1, 0xe410ab, "short_jump"),
    (0xe410a9, &[0x75], 2, 1, 0xe4110d, "short_jump"),
    (0xe410ee, &[0xe8], 5, 1, 0xe412e0, "direct_call"),
    (0xe410f9, &[0x75], 2, 1, 0xe41090, "short_jump"),
    (0xe425b7, &[0xe8], 5, 1, 0xe42b80, "direct_call"),
    (0xe425cc, &[0x75], 2, 1, 0xe42601, "short_jump"),
    (0xe425dd, &[0x0f, 0x84], 6, 2, 0xe42692, "conditional_rel32"),
    (0xe425f1, &[0x0f, 0x85], 6, 2, 0xe42692, "conditional_rel32"),
    (0xe425fb, &[0x0f, 0x84], 6, 2, 0xe42694, "conditional_rel32"),
    (0xe4260d, &[0x72], 2, 1, 0xe42624, "short_jump"),
    (0xe42936, &[0xe8], 5, 1, 0xe42b80, "direct_call"),
    (0xe4294b, &[0x75], 2, 1, 0xe42978, "short_jump"),
    (0xe4295c, &[0x0f, 0x84], 6, 2, 0xe429ef, "conditional_rel32"),
    (0xe42970, &[0x75], 2, 1, 0xe429ef, "short_jump"),
    (0xe42976, &[0x74], 2, 1, 0xe429f1, "short_jump"),
    (0xe42984, &[0x72], 2, 1, 0xe4299b, "short_jump"),
    (0xe41aed, &[0xeb], 2, 1, 0xe41ab0, "short_jump"),
    (0xe41eaa, &[0xeb], 2, 1, 0xe41e6d, "short_jump"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> Vec<u8> {
        let mut b = vec![0; 0x1240000];
        for &(r, code) in BLOCKS {
            let p = r as usize - 0xc00;
            b[p..p + code.len()].copy_from_slice(code);
        }
        b[0x106c428 - 0xc00..0x106c430 - 0xc00].copy_from_slice(&0x1400967b0u64.to_le_bytes());
        b[0x967b0 - 0xc00..0x967b3 - 0xc00].copy_from_slice(&[0xc2, 0, 0]);
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
                    virtual_size: 0x2b0000,
                    raw: 0xf95400,
                    size: 0x2b0000,
                    flags: 0x40000040,
                },
            ],
            functions: vec![],
            relocations: Default::default(),
        }
    }

    #[test]
    fn consumer_execute_skip_destructor_and_tag_bytes_reject_each_mutation() {
        let mut b = image();
        blocks(&pe(&b)).unwrap();
        for &(r, code) in BLOCKS {
            for i in 0..code.len() {
                let p = r as usize - 0xc00 + i;
                b[p] ^= 1;
                assert!(blocks(&pe(&b)).is_err(), "instruction {r:x}+{i}");
                b[p] ^= 1;
            }
        }
    }

    #[test]
    fn consumer_branch_directions_and_signed_targets_reject_mutations() {
        let mut b = image();
        references(&pe(&b)).unwrap();
        for &(r, prefix, length, offset, _, _) in BRANCHES {
            for i in (0..prefix.len()).chain(offset..length) {
                let p = r as usize - 0xc00 + i;
                b[p] ^= 1;
                assert!(references(&pe(&b)).is_err(), "branch {r:x}+{i}");
                b[p] ^= 1;
            }
        }
        assert!(rel_target(0, &[0xe9, 0xf0, 0xff, 0xff, 0xff], 0xe9, 5, 1).is_err());
        assert!(rel_target(u32::MAX, &[0xe9, 1, 0, 0, 0], 0xe9, 5, 1).is_err());
        assert_eq!(relative_target(0x100, &[0x75, 0xf0], 2, 1).unwrap(), 0xf2);
        assert_eq!(relative_target(0x100, &[0x74, 0x10], 2, 1).unwrap(), 0x112);
        assert!(relative_target(0, &[0x75, 0xf0], 2, 1).is_err());
        assert!(relative_target(u32::MAX, &[0x75, 0], 2, 1).is_err());
        assert!(relative_target(0, &[0x75], 2, 1).is_err());
    }

    #[test]
    fn consumer_code_data_affinity_and_truncation_are_checked() {
        let mut b = image();
        references(&pe(&b)).unwrap();
        b[0x106c428 - 0xc00] ^= 1;
        assert!(references(&pe(&b)).is_err());
        b[0x106c428 - 0xc00] ^= 1;
        b[0x967b0 - 0xc00] ^= 1;
        assert!(references(&pe(&b)).is_err());
        let mut p = pe(&b);
        p.sections[0].flags &= !0x20000000;
        assert!(blocks(&p).is_err());
        let mut p = pe(&b);
        p.sections[1].flags |= 0x20000000;
        assert!(references(&p).is_err());
        assert!(blocks(&pe(&b[..0xe410a8 - 0xc00])).is_err());
    }

    #[test]
    fn consumer_complete_frames_reject_unwitnessed_tail_pdata_and_unwind_mutations() {
        // Synthetic complete bodies exercise the same frame/CHAININFO checker
        // without requiring a local game installation or redistributing it.
        let mut b = image();
        let mut functions = Vec::new();
        for s in FRAMES {
            b[s.begin as usize - 0xc00..s.end as usize - 0xc00].fill(0x90);
            for &(begin, end, unwind, hex) in s.fragments {
                functions.push(Function { begin, end, unwind });
                for i in (0..hex.len()).step_by(2) {
                    b[unwind as usize - 0xc00 + i / 2] =
                        u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
                }
            }
        }
        functions.sort_by_key(|f| f.begin);
        for s in FRAMES {
            let sha = format!(
                "{:x}",
                Sha256::digest(&b[s.begin as usize - 0xc00..s.end as usize - 0xc00])
            );
            let fixture = Frame {
                sha: Box::leak(sha.into_boxed_str()),
                begin: s.begin,
                end: s.end,
                fragments: s.fragments,
            };
            let mapped = || {
                let mut p = pe(&b);
                p.functions = functions.clone();
                p
            };
            frame(&mapped(), &fixture).unwrap();
            let mut p = mapped();
            p.functions
                .retain(|f| f.begin != s.fragments.last().unwrap().0);
            assert!(frame(&p, &fixture).is_err(), "missing pdata {:x}", s.begin);
            let tail = s.end as usize - 0xc00 - 1;
            b[tail] ^= 1;
            let mut p = pe(&b);
            p.functions = functions.clone();
            assert!(frame(&p, &fixture).is_err(), "complete body {:x}", s.begin);
            b[tail] ^= 1;
            for &(_, _, unwind, hex) in s.fragments {
                let p = unwind as usize - 0xc00 + hex.len() / 2 - 1;
                b[p] ^= 1;
                let mut mapped = pe(&b);
                mapped.functions = functions.clone();
                assert!(frame(&mapped, &fixture).is_err(), "unwind {unwind:x}");
                b[p] ^= 1;
            }
        }
    }

    #[test]
    fn consumer_selector_requires_both_complete_pinned_buffers() {
        assert!(audit_queue_consumer(b"MZ", b"PE")
            .unwrap_err()
            .contains("pinned"));
        assert!(require_image_pair(CORE_SHA, "wrong-host").is_err());
        assert!(require_image_pair("wrong-core", HOST_SHA).is_err());
    }
}
