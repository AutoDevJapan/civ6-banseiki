//! Portable wire layer of the Oath diagnostic controller: byte layouts of the
//! runtime DLL (trace v5, 48 byte config, 64 byte receipt), PE export parsing
//! and the JSON projection. It touches no process and no file, so it is
//! tested on any host. Every number here is observation metadata: nothing in
//! this module produces, accepts or adjusts a quantity.
// The Windows controller consumes this wire format; on other hosts only its
// portable parser tests use it.
#![allow(dead_code)]
use serde_json::{json, Value};

pub(crate) const RUNTIME_VERSION: u32 = 0x0134_0002;
pub(crate) const MODE_DIAGNOSTIC: u32 = 1;
pub(crate) const MAX_SECONDS: u32 = 300;
pub(crate) const CONFIG_SIZE: usize = 48;
pub(crate) const RECEIPT_SIZE: usize = 64;

pub(crate) const TRACE_MAGIC: u32 = 0x4a41_434f;
pub(crate) const TRACE_VERSION: u32 = 5;
pub(crate) const CAPACITY: usize = 1024;
pub(crate) const HEADER_SIZE: usize = 64;
pub(crate) const ROW_SIZE: usize = 136;
pub(crate) const TRACE_SIZE: usize = HEADER_SIZE + CAPACITY * ROW_SIZE;
/// Upper bound of any raw trace read.
pub(crate) const MAX_TRACE_READ: usize = 256 * 1024;

// Trace header offsets (repr(C), v4).
const H_NEXT: usize = 8;
const H_DROPPED: usize = 12;
const H_IN_FLIGHT: usize = 16;
const H_COMPLETED: usize = 20;
const H_STOPPED: usize = 24;
const H_IMAGE_BASE: usize = 32;
const H_FILTERED: usize = 40;
const H_DEADLINE_HIT: usize = 56;

pub(crate) const EXPORT_INSTALL: &str = "OathRuntimeInstall";
pub(crate) const EXPORT_START: &str = "OathRuntimeStartObservation";
pub(crate) const EXPORT_STOP: &str = "OathRuntimeStopObservation";
pub(crate) const EXPORT_RECEIPT: &str = "OathRuntimeReadReceipt";
pub(crate) const EXPORTS: [&str; 4] = [EXPORT_INSTALL, EXPORT_START, EXPORT_STOP, EXPORT_RECEIPT];

#[cfg(all(test, windows, target_arch = "x86_64"))]
#[test]
fn controller_layout_matches_the_compiled_runtime_abi() {
    let (header, row, trace, header_offsets, row_offsets) =
        civ6_oath_runtime_rs::OathTrace::wire_layout();
    assert_eq!((header, row, trace), (HEADER_SIZE, ROW_SIZE, TRACE_SIZE));
    assert_eq!(header_offsets, [8, 12, 16, 20, 24, 32, 40, 48, 56]);
    assert_eq!(
        row_offsets,
        [
            0, 4, 8, 12, 16, 20, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 124, 128,
            132
        ]
    );
    assert_eq!(
        std::mem::size_of::<civ6_oath_runtime_rs::runtime::OathRuntimeConfig>(),
        CONFIG_SIZE
    );
    assert_eq!(
        std::mem::size_of::<civ6_oath_runtime_rs::runtime::OathReceipt>(),
        RECEIPT_SIZE
    );
}

pub(crate) fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}
pub(crate) fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}
fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

/// Runtime status codes (OathStatus). Unknown values are reported, never
/// treated as success or as a zero contribution.
pub(crate) fn status_name(status: u32) -> String {
    match status {
        0 => "ok".into(),
        1 => "invalid_input".into(),
        2 => "process_identity".into(),
        3 => "image_identity".into(),
        4 => "entry_bytes".into(),
        5 => "hook_failure".into(),
        6 => "critical".into(),
        7 => "wrong_state".into(),
        8 => "not_wired".into(),
        99 => "panic".into(),
        other => format!("unknown_status_{other}"),
    }
}

/// The 48 byte request. Both out fields stay zero on input.
pub(crate) fn encode_config(
    pid: u32,
    max_seconds: u32,
    created_filetime: u64,
) -> [u8; CONFIG_SIZE] {
    let mut out = [0u8; CONFIG_SIZE];
    out[0..4].copy_from_slice(&RUNTIME_VERSION.to_le_bytes());
    out[4..8].copy_from_slice(&(CONFIG_SIZE as u32).to_le_bytes());
    out[8..12].copy_from_slice(&pid.to_le_bytes());
    out[12..16].copy_from_slice(&max_seconds.to_le_bytes());
    out[16..24].copy_from_slice(&created_filetime.to_le_bytes());
    out[24..28].copy_from_slice(&MODE_DIAGNOSTIC.to_le_bytes());
    out
}

/// What a successful install may have changed in the config block: only the
/// two out fields. Returns (trace address, trace size).
pub(crate) fn check_installed_config(
    sent: &[u8; CONFIG_SIZE],
    after: &[u8],
) -> Result<(u64, u32), String> {
    if after.len() != CONFIG_SIZE {
        return Err("config block has the wrong length".into());
    }
    // 0..32 input fields, 32..40 trace address, 40..44 trace size, 44..48 reserved
    if after[..32] != sent[..32] || after[44..48] != sent[44..48] {
        return Err("install changed a config field other than the two outputs".into());
    }
    let address = u64_at(after, 32).unwrap_or(0);
    let size = u32_at(after, 40).unwrap_or(0);
    if address == 0 || !address.is_multiple_of(8) {
        return Err("install returned no usable trace address".into());
    }
    if size as usize != TRACE_SIZE {
        return Err(format!(
            "install returned trace size {size}, expected {TRACE_SIZE}"
        ));
    }
    Ok((address, size))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Receipt {
    pub phase: u32,
    pub trace_address: u64,
    pub trace_size: u32,
    pub max_seconds: u32,
    pub rows: u32,
    pub completed: u32,
    pub dropped: u32,
    pub filtered: u32,
    pub in_flight: u32,
    pub expired: u32,
}

pub(crate) fn decode_receipt(bytes: &[u8]) -> Result<Receipt, String> {
    if bytes.len() != RECEIPT_SIZE {
        return Err("receipt has the wrong length".into());
    }
    let f = |at| u32_at(bytes, at).unwrap_or(u32::MAX);
    if f(0) != RUNTIME_VERSION || f(4) as usize != RECEIPT_SIZE {
        return Err("receipt version or size differs".into());
    }
    // quantity_accepted (offset 56) and reserved (60) must be zero.
    if f(56) != 0 || f(60) != 0 {
        return Err("receipt claims an accepted quantity".into());
    }
    Ok(Receipt {
        phase: f(8),
        trace_address: u64_at(bytes, 16).unwrap_or(0),
        trace_size: f(24),
        max_seconds: f(28),
        rows: f(32),
        completed: f(36),
        dropped: f(40),
        filtered: f(44),
        in_flight: f(48),
        expired: f(52),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TraceView {
    pub bytes: Vec<u8>,
    pub rows: usize,
}
impl TraceView {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != TRACE_SIZE {
            return Err("trace copy has the wrong length".into());
        }
        if u32_at(bytes, 0) != Some(TRACE_MAGIC) || u32_at(bytes, 4) != Some(TRACE_VERSION) {
            return Err("trace magic or version differs".into());
        }
        let next = u32_at(bytes, H_NEXT).unwrap_or(u32::MAX) as usize;
        if next > CAPACITY {
            return Err("trace reservation count exceeds the finite capacity".into());
        }
        Ok(Self {
            bytes: bytes.to_vec(),
            rows: next,
        })
    }
    pub fn in_flight(&self) -> u32 {
        u32_at(&self.bytes, H_IN_FLIGHT).unwrap_or(u32::MAX)
    }
    fn header(&self, at: usize) -> u32 {
        u32_at(&self.bytes, at).unwrap_or(u32::MAX)
    }
    fn row(&self, index: usize) -> &[u8] {
        let start = HEADER_SIZE + index * ROW_SIZE;
        &self.bytes[start..start + ROW_SIZE]
    }
}

fn hex(value: u64) -> String {
    format!("0x{value:x}")
}

fn call_name(value: u32) -> Value {
    match value {
        1 => json!("apply"),
        2 => json!("remove"),
        other => json!(format!("unknown_{other}")),
    }
}

fn copy_flag_names(flags: u32) -> Vec<&'static str> {
    let mut names = Vec::new();
    if flags & 1 != 0 {
        names.push("effect_vtable");
    }
    if flags & 2 != 0 {
        names.push("subject_pointer");
    }
    if flags & 4 != 0 {
        names.push("subject_object_present");
    }
    names
}

/// One row. A row that changed between the two copies, or that was never
/// published, is Unknown: it contributes no state claim at all.
fn project_row(index: usize, row: &[u8], stable: bool) -> Value {
    let u = |at| u32_at(row, at).unwrap_or(0);
    let q = |at| u64_at(row, at).unwrap_or(0);
    let state = match (stable, u(0)) {
        (false, _) => "unknown_unstable",
        (true, 0) => "unknown_unpublished",
        (true, 1) => "incomplete",
        (true, 2) => "completed",
        (true, 3) => "unwound",
        (true, _) => "unknown_state",
    };
    let completed = stable && u(0) == 2;
    let metadata_flags = u(124);
    let identity_copied =
        stable && matches!(u(0), 1..=3) && metadata_flags & !3 == 0 && metadata_flags & 1 != 0;
    let city_copied = identity_copied && metadata_flags & 2 != 0;
    json!({
        "sequence": index + 1,
        "call": call_name(u(4)),
        "state": state,
        "thread_id": u(8),
        "instance_id": u(12),
        "copy_flags": u(16),
        "copy_flag_names": copy_flag_names(u(16)),
        "completion_order": u(20),
        "raw_r8": hex(q(24)),
        "effect": hex(q(32)),
        "subject": hex(q(40)),
        "caller_return": hex(q(48)),
        "caller_rbp": hex(q(56)),
        "caller_rdi": hex(q(64)),
        "effect_vtable": hex(q(72)),
        "subject_object": hex(q(80)),
        "subject_control": hex(q(88)),
        // Only a completed call has a result; AL is the boolean reading.
        "original_rax": if completed { json!(hex(q(96))) } else { Value::Null },
        "original_al": if completed { json!(q(96) & 0xff) } else { Value::Null },
        "call_entry_metadata": {
            "identity_copied": identity_copied,
            "instance": if identity_copied { json!(hex(q(104))) } else { Value::Null },
            "definition": if identity_copied { json!(hex(q(112))) } else { Value::Null },
            "declared_percent": if identity_copied { json!(u(120) as i32) } else { Value::Null },
            "indicated_city": if city_copied { json!({"owner":u(128),"full_id":u(132),"resolved_receiver_proved":false}) } else { Value::Null },
            "lifetime_proved":false,"quantity_accepted":false,
        },
        "authority": "observation_only",
    })
}

/// Projects two raw copies of the trace. Rows must agree byte for byte to be
/// stable. The result is complete only when the trace is stopped, nothing is
/// in flight and every row agreed.
pub(crate) fn project_trace(first: &TraceView, second: &TraceView, receipt: &Receipt) -> Value {
    let rows = second.rows;
    let mut projected = Vec::with_capacity(rows);
    let mut unstable = 0usize;
    let mut open = 0usize;
    for index in 0..rows {
        let stable = index < first.rows && first.row(index) == second.row(index);
        if !stable {
            unstable += 1;
        }
        let row = project_row(index, second.row(index), stable);
        if row["state"] != "completed" && row["state"] != "unwound" {
            open += 1;
        }
        projected.push(row);
    }
    let stopped = second.header(H_STOPPED) == 2;
    let header_agrees = first.bytes[..HEADER_SIZE] == second.bytes[..HEADER_SIZE];
    let complete =
        stopped && second.in_flight() == 0 && header_agrees && unstable == 0 && open == 0;
    json!({
        "trace_version": TRACE_VERSION,
        "trace_stopped": stopped,
        "complete": complete,
        "rows": projected,
        "row_count": rows,
        "unstable_rows": unstable,
        "open_rows": open,
        "header_agrees": header_agrees,
        "in_flight": second.in_flight(),
        "completed_counter": second.header(H_COMPLETED),
        "dropped": second.header(H_DROPPED),
        "filtered": second.header(H_FILTERED),
        "deadline_hit": second.header(H_DEADLINE_HIT) != 0,
        "image_base": hex(u64_at(&second.bytes, H_IMAGE_BASE).unwrap_or(0)),
        "receipt": {
            "phase": receipt.phase,
            "rows": receipt.rows,
            "completed": receipt.completed,
            "dropped": receipt.dropped,
            "filtered": receipt.filtered,
            "in_flight": receipt.in_flight,
            "expired": receipt.expired,
            "max_seconds": receipt.max_seconds,
        },
        // A completed Apply is an observation of the call, never a quantity.
        "accepted": false,
        "quantity_accepted": false,
        "authority": "observation_only",
    })
}

/// Facts of a PE image that the loader checks need.
#[derive(Clone, Debug)]
pub(crate) struct PeImage {
    pub size_of_image: u32,
    pub size_of_headers: u32,
    /// File offset of the optional header's ImageBase (rewritten by the loader).
    pub image_base_offset: usize,
    pub exports: Vec<(String, u32)>,
    sections: Vec<(u32, u32, u32, u32, u32)>, // va, vsize, raw, rawsize, characteristics
}
impl PeImage {
    pub fn export(&self, name: &str) -> Option<u32> {
        let mut found = self.exports.iter().filter(|(n, _)| n == name);
        let first = found.next()?;
        found.next().is_none().then_some(first.1)
    }
    /// File offset of `len` bytes at `rva`, only if the whole width lies in
    /// the raw data of one section (a read never straddles two sections).
    fn offset_for(&self, rva: u32, len: usize) -> Option<usize> {
        let len = u32::try_from(len).ok().filter(|n| *n > 0)?;
        self.sections.iter().find_map(|&(va, _, raw, rawsize, _)| {
            let rel = rva.checked_sub(va)?;
            let end = rel.checked_add(len)?;
            (end <= rawsize)
                .then(|| u64::from(raw) + u64::from(rel))
                .and_then(|offset| usize::try_from(offset).ok())
        })
    }
    /// The bytes at an RVA, only if they lie inside one section's raw data.
    pub fn bytes_at<'a>(&self, file: &'a [u8], rva: u32, len: usize) -> Option<&'a [u8]> {
        let start = self.offset_for(rva, len)?;
        file.get(start..start.checked_add(len)?)
    }
    /// Up to `max` bytes from `rva` to the end of that section's raw data.
    fn tail<'a>(&self, file: &'a [u8], rva: u32, max: usize) -> Option<&'a [u8]> {
        self.sections.iter().find_map(|&(va, _, raw, rawsize, _)| {
            let rel = rva.checked_sub(va).filter(|rel| *rel < rawsize)?;
            let start = usize::try_from(u64::from(raw) + u64::from(rel)).ok()?;
            let end = start
                .checked_add(max)?
                .min(usize::try_from(u64::from(raw) + u64::from(rawsize)).ok()?);
            file.get(start..end)
        })
    }
    pub fn rva_is_executable(&self, rva: u32) -> bool {
        self.sections.iter().any(|&(va, vsize, _, rawsize, ch)| {
            ch & 0x2000_0000 != 0 && rva >= va && rva - va < vsize.max(rawsize)
        })
    }
}

pub(crate) fn parse_pe(bytes: &[u8]) -> Result<PeImage, String> {
    let bad = |what: &str| format!("not an acceptable Win64 DLL: {what}");
    let pe = u32_at(bytes, 0x3c).ok_or_else(|| bad("no PE offset"))? as usize;
    if bytes.get(0..2) != Some(b"MZ") || bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err(bad("signature"));
    }
    let machine = u16_at(bytes, pe + 4).ok_or_else(|| bad("header"))?;
    let section_count = u16_at(bytes, pe + 6).ok_or_else(|| bad("header"))? as usize;
    let optional_size = u16_at(bytes, pe + 20).ok_or_else(|| bad("header"))? as usize;
    let characteristics = u16_at(bytes, pe + 22).ok_or_else(|| bad("header"))?;
    let opt = pe + 24;
    if machine != 0x8664 || u16_at(bytes, opt) != Some(0x20b) || characteristics & 0x2000 == 0 {
        return Err(bad("machine, optional header or DLL flag"));
    }
    if section_count == 0 || section_count > 96 || optional_size < 112 + 16 {
        return Err(bad("section table"));
    }
    let size_of_image = u32_at(bytes, opt + 56).ok_or_else(|| bad("size"))?;
    let size_of_headers = u32_at(bytes, opt + 60).ok_or_else(|| bad("size"))?;
    let export_rva = u32_at(bytes, opt + 112).ok_or_else(|| bad("export dir"))?;
    let export_size = u32_at(bytes, opt + 116).ok_or_else(|| bad("export dir"))?;
    let mut sections: Vec<(u32, u32, u32, u32, u32)> = Vec::new();
    for n in 0..section_count {
        let at = opt + optional_size + n * 40;
        let f = |o| u32_at(bytes, at + o).ok_or_else(|| bad("section"));
        let (vsize, va, rawsize, raw, ch) = (f(8)?, f(12)?, f(16)?, f(20)?, f(36)?);
        if u64::from(raw) + u64::from(rawsize) > bytes.len() as u64
            || va as u64 + vsize as u64 > size_of_image as u64
        {
            return Err(bad("section outside the file or image"));
        }
        // Raw data and virtual ranges of different sections must not overlap.
        let overlaps = |a: (u64, u64), b: (u64, u64)| a.0 < b.1 && b.0 < a.1;
        for &(ova, ovsize, oraw, orawsize, _) in &sections {
            if overlaps(
                (u64::from(va), u64::from(va) + u64::from(vsize.max(1))),
                (u64::from(ova), u64::from(ova) + u64::from(ovsize.max(1))),
            ) || (rawsize > 0
                && orawsize > 0
                && overlaps(
                    (u64::from(raw), u64::from(raw) + u64::from(rawsize)),
                    (u64::from(oraw), u64::from(oraw) + u64::from(orawsize)),
                ))
            {
                return Err(bad("overlapping sections"));
            }
        }
        sections.push((va, vsize, raw, rawsize, ch));
    }
    let mut image = PeImage {
        size_of_image,
        size_of_headers,
        image_base_offset: opt + 24,
        exports: Vec::new(),
        sections,
    };
    if export_rva == 0 || export_size < 40 {
        return Err(bad("no export directory"));
    }
    let dir = image
        .offset_for(export_rva, 40)
        .ok_or_else(|| bad("export directory"))?;
    let count = u32_at(bytes, dir + 20).ok_or_else(|| bad("export"))? as usize;
    let names = u32_at(bytes, dir + 24).ok_or_else(|| bad("export"))? as usize;
    let functions = u32_at(bytes, dir + 28).ok_or_else(|| bad("export"))?;
    let name_table = u32_at(bytes, dir + 32).ok_or_else(|| bad("export"))?;
    let ordinals = u32_at(bytes, dir + 36).ok_or_else(|| bad("export"))?;
    if names > 4096 || count > 4096 {
        return Err(bad("export table too large"));
    }
    for n in 0..names {
        // Every table element is read at its full width inside one section.
        let element = |table: u32, width: u32, index: u32, what: &str| {
            let at = width
                .checked_mul(index)
                .and_then(|off| table.checked_add(off))
                .ok_or_else(|| bad(what))?;
            image
                .bytes_at(bytes, at, width as usize)
                .ok_or_else(|| bad(what))
        };
        let name_rva = element(name_table, 4, n as u32, "export name table")?
            .try_into()
            .map(u32::from_le_bytes)
            .ok();
        let ordinal = element(ordinals, 2, n as u32, "export ordinal table")?
            .try_into()
            .map(u16::from_le_bytes)
            .ok();
        let (Some(name_rva), Some(ordinal)) = (name_rva, ordinal) else {
            return Err(bad("export name tables"));
        };
        let tail = image
            .tail(bytes, name_rva, 128)
            .ok_or_else(|| bad("export name"))?;
        let end = tail
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| bad("unterminated export name"))?;
        let name = String::from_utf8_lossy(&tail[..end]).into_owned();
        if ordinal as usize >= count {
            return Err(bad("export ordinal"));
        }
        let rva = u32::from_le_bytes(
            element(functions, 4, u32::from(ordinal), "export function table")?
                .try_into()
                .map_err(|_| bad("export function table"))?,
        );
        if rva >= export_rva && u64::from(rva) < u64::from(export_rva) + u64::from(export_size) {
            return Err(bad("forwarded export"));
        }
        image.exports.push((name, rva));
    }
    Ok(image)
}

/// RVAs of the four runtime exports; each must be unique and executable.
pub(crate) fn runtime_exports(image: &PeImage) -> Result<[u32; 4], String> {
    let mut out = [0u32; 4];
    for (slot, name) in out.iter_mut().zip(EXPORTS) {
        let rva = image
            .export(name)
            .ok_or_else(|| format!("export {name} is missing or duplicated"))?;
        if rva == 0 || rva >= image.size_of_image || !image.rva_is_executable(rva) {
            return Err(format!("export {name} is not inside executable code"));
        }
        *slot = rva;
    }
    Ok(out)
}

/// Bytes the loader must not have changed: headers with ImageBase masked.
pub(crate) fn header_matches(disk: &[u8], remote: &[u8], image: &PeImage) -> bool {
    let len = image.size_of_headers as usize;
    if disk.len() < len || remote.len() < len {
        return false;
    }
    let mask = image.image_base_offset..image.image_base_offset + 8;
    (0..len).all(|i| mask.contains(&i) || disk[i] == remote[i])
}

pub(crate) fn lower_hex_sha(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A module as the loader or ToolHelp reports it, path already canonical.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModuleFact {
    pub path: String,
    pub base: usize,
    pub size: usize,
}

/// Decode only a local RIP-relative import jump, and require its pointer slot
/// to be wholly inside that image. Never follow a game's modified entry.
pub(crate) fn local_import_slot(
    function: usize,
    owner: &ModuleFact,
    code: &[u8],
) -> Result<Option<usize>, String> {
    let end = owner
        .base
        .checked_add(owner.size)
        .ok_or("local image bounds overflow")?;
    let (length, displacement) = if code.starts_with(&[0x48, 0xff, 0x25]) {
        (7, code.get(3..7))
    } else if code.starts_with(&[0xff, 0x25]) {
        (6, code.get(2..6))
    } else {
        return Ok(None);
    };
    let displacement = i32::from_le_bytes(
        displacement
            .ok_or("truncated local import jump")?
            .try_into()
            .unwrap(),
    );
    let next = function
        .checked_add(length)
        .ok_or("local import jump overflow")?;
    if function < owner.base || next > end {
        return Err("local import jump outside its image".into());
    }
    let slot = next
        .checked_add_signed(displacement as isize)
        .ok_or("local import slot overflow")?;
    if slot < owner.base || slot.checked_add(8).is_none_or(|last| last > end) {
        return Err("local import slot outside its image".into());
    }
    Ok(Some(slot))
}

fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.strip_prefix(r"\\?\")
            .unwrap_or(s)
            .replace('/', "\\")
            .to_ascii_lowercase()
    };
    norm(a) == norm(b)
}

/// The module that really owns a function address locally (from VirtualQuery
/// plus GetModuleFileNameW) must exist in the remote process with the same
/// path, base and size, and the address must lie inside it. A forwarded
/// export therefore proves its real owner, never the module it was looked up
/// in. Returns the index of the remote module.
pub(crate) fn function_owner_in_remote(
    function: usize,
    local_owner: &ModuleFact,
    remote: &[ModuleFact],
) -> Result<usize, String> {
    let end = local_owner
        .base
        .checked_add(local_owner.size)
        .ok_or("owner range overflows")?;
    if local_owner.size == 0 || function < local_owner.base || function >= end {
        return Err("function address is outside its reported owner module".into());
    }
    let mut hits = remote
        .iter()
        .enumerate()
        .filter(|(_, m)| same_path(&m.path, &local_owner.path));
    let (index, module) = hits
        .next()
        .ok_or("owner module is not loaded in the remote process")?;
    if hits.next().is_some() {
        return Err("owner module path appears more than once remotely".into());
    }
    if module.base != local_owner.base || module.size != local_owner.size {
        return Err("owner module has a different base or size remotely".into());
    }
    Ok(index)
}

/// Outcome of the module search after an unobserved-result LoadLibraryW.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LoadVerdict {
    Loaded(usize),
    /// Nothing proves the load either way: the caller ends the game.
    Unknown(&'static str),
}

/// The thread result is only the low half of an HMODULE, and 0 is a legal low
/// half, so it never decides on its own: the single new matching module does.
pub(crate) fn judge_load(low: u32, expected_size: usize, matching: &[ModuleFact]) -> LoadVerdict {
    match matching {
        [] if low == 0 => LoadVerdict::Unknown("LoadLibraryW returned 0 and no module appeared"),
        [] => LoadVerdict::Unknown("LoadLibraryW reported a module that is not listed"),
        [one] if one.size == expected_size && one.base as u32 == low => {
            LoadVerdict::Loaded(one.base)
        }
        [_] => LoadVerdict::Unknown(
            "the loaded module differs from the pinned DLL or the reported base",
        ),
        _ => LoadVerdict::Unknown("the DLL path is loaded more than once"),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn only_bounded_local_import_jumps_can_name_a_pointer_slot() {
        let owner = ModuleFact {
            path: "kernel32.dll".into(),
            base: 0x1000,
            size: 0x1000,
        };
        let jump = |displacement: i32| {
            let mut code = vec![0x48, 0xff, 0x25];
            code.extend(displacement.to_le_bytes());
            code
        };
        assert_eq!(
            local_import_slot(0x1100, &owner, &jump(0x99)).unwrap(),
            Some(0x11a0)
        );
        assert_eq!(
            local_import_slot(0x1100, &owner, &jump(-7)).unwrap(),
            Some(0x1100)
        );
        assert!(local_import_slot(0x1100, &owner, &jump(-0x108)).is_err());
        assert!(local_import_slot(0x1100, &owner, &jump(0xef2)).is_err());
        assert!(local_import_slot(0x1ffa, &owner, &jump(0)).is_err());
        assert!(local_import_slot(0x1100, &owner, &[0x48, 0xff, 0x25]).is_err());
        assert_eq!(
            local_import_slot(0x1100, &owner, &[0xe9, 0, 0, 0, 0]).unwrap(),
            None
        );
        assert_eq!(
            local_import_slot(0x1100, &owner, &[0x48, 0x83, 0xec, 0x28]).unwrap(),
            None
        );
    }

    #[test]
    fn layout_constants_follow_the_v5_trace() {
        assert_eq!(HEADER_SIZE + ROW_SIZE * CAPACITY, TRACE_SIZE);
        assert_eq!(TRACE_SIZE, 139_328);
        const { assert!(TRACE_SIZE <= MAX_TRACE_READ) };
        assert_eq!(H_NEXT + 4, H_DROPPED);
        assert_eq!(H_DEADLINE_HIT, 56);
        assert_eq!(ROW_SIZE, 17 * 8);
    }

    #[test]
    fn config_roundtrip_accepts_only_the_two_outputs() {
        let sent = encode_config(42, 30, 0x01d9_0000_0000_0001);
        assert_eq!(u32_at(&sent, 0), Some(RUNTIME_VERSION));
        assert_eq!(u32_at(&sent, 24), Some(MODE_DIAGNOSTIC));
        let mut after = sent;
        after[32..40].copy_from_slice(&0x1234_5000u64.to_le_bytes());
        after[40..44].copy_from_slice(&(TRACE_SIZE as u32).to_le_bytes());
        assert_eq!(
            check_installed_config(&sent, &after),
            Ok((0x1234_5000, TRACE_SIZE as u32))
        );
        for (at, value) in [(9usize, 1u8), (17, 1), (28, 1), (46, 1), (0, 9)] {
            let mut bad = after;
            bad[at] ^= value;
            assert!(check_installed_config(&sent, &bad).is_err(), "byte {at}");
        }
        let mut no_trace = sent;
        no_trace[40..44].copy_from_slice(&(TRACE_SIZE as u32).to_le_bytes());
        assert!(check_installed_config(&sent, &no_trace).is_err());
        let mut wrong_size = after;
        wrong_size[40..44].copy_from_slice(&1u32.to_le_bytes());
        assert!(check_installed_config(&sent, &wrong_size).is_err());
        let mut unaligned = after;
        unaligned[32..40].copy_from_slice(&0x1234_5001u64.to_le_bytes());
        assert!(check_installed_config(&sent, &unaligned).is_err());
    }

    fn receipt_bytes() -> [u8; RECEIPT_SIZE] {
        let mut out = [0u8; RECEIPT_SIZE];
        out[0..4].copy_from_slice(&RUNTIME_VERSION.to_le_bytes());
        out[4..8].copy_from_slice(&(RECEIPT_SIZE as u32).to_le_bytes());
        out[8..12].copy_from_slice(&2u32.to_le_bytes());
        out
    }

    #[test]
    fn a_receipt_that_accepts_a_quantity_is_refused() {
        assert!(decode_receipt(&receipt_bytes()).is_ok());
        let mut accepted = receipt_bytes();
        accepted[56] = 1;
        assert!(decode_receipt(&accepted).is_err());
        let mut reserved = receipt_bytes();
        reserved[60] = 1;
        assert!(decode_receipt(&reserved).is_err());
        let mut version = receipt_bytes();
        version[0] ^= 1;
        assert!(decode_receipt(&version).is_err());
        assert!(decode_receipt(&receipt_bytes()[..60]).is_err());
    }

    fn blank_trace(stopped: u32) -> Vec<u8> {
        let mut bytes = vec![0u8; TRACE_SIZE];
        bytes[0..4].copy_from_slice(&TRACE_MAGIC.to_le_bytes());
        bytes[4..8].copy_from_slice(&TRACE_VERSION.to_le_bytes());
        bytes[H_STOPPED..H_STOPPED + 4].copy_from_slice(&stopped.to_le_bytes());
        bytes
    }
    fn set_row(bytes: &mut [u8], index: usize, state: u32, call: u32, rax: u64) {
        let at = HEADER_SIZE + index * ROW_SIZE;
        bytes[at..at + 4].copy_from_slice(&state.to_le_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&call.to_le_bytes());
        bytes[at + 12..at + 16].copy_from_slice(&0x0aa1u32.to_le_bytes());
        bytes[at + 56..at + 64].copy_from_slice(&0xaaaa_bbbbu64.to_le_bytes());
        bytes[at + 64..at + 72].copy_from_slice(&0xcccc_ddddu64.to_le_bytes());
        bytes[at + 96..at + 104].copy_from_slice(&rax.to_le_bytes());
        let next = (index as u32 + 1).to_le_bytes();
        bytes[H_NEXT..H_NEXT + 4].copy_from_slice(&next);
    }
    #[test]
    fn call_entry_metadata_is_nullable_and_never_establishes_a_receiver_or_lifetime() {
        let mut bytes = blank_trace(2);
        set_row(&mut bytes, 0, 2, 1, 1);
        let at = HEADER_SIZE;
        bytes[at + 104..at + 112].copy_from_slice(&0x2000u64.to_le_bytes());
        bytes[at + 112..at + 120].copy_from_slice(&0x3000u64.to_le_bytes());
        bytes[at + 120..at + 124].copy_from_slice(&(-15i32).to_le_bytes());
        bytes[at + 124..at + 128].copy_from_slice(&3u32.to_le_bytes());
        bytes[at + 132..at + 136].copy_from_slice(&65536u32.to_le_bytes());
        let row = &bytes[at..at + ROW_SIZE];
        let value = project_row(0, row, true);
        let meta = &value["call_entry_metadata"];
        assert_eq!(meta["declared_percent"], -15);
        assert_eq!(meta["indicated_city"]["full_id"], 65536);
        assert_eq!(meta["indicated_city"]["resolved_receiver_proved"], false);
        assert_eq!(meta["lifetime_proved"], false);
        assert_eq!(meta["quantity_accepted"], false);
        assert!(project_row(0, row, false)["call_entry_metadata"]["declared_percent"].is_null());
        bytes[at + 124..at + 128].copy_from_slice(&0u32.to_le_bytes());
        assert!(
            project_row(0, &bytes[at..at + ROW_SIZE], true)["call_entry_metadata"]
                ["declared_percent"]
                .is_null()
        );
        bytes[at + 124..at + 128].copy_from_slice(&0x83u32.to_le_bytes());
        assert_eq!(
            project_row(0, &bytes[at..at + ROW_SIZE], true)["call_entry_metadata"]
                ["identity_copied"],
            false
        );
    }
    fn receipt() -> Receipt {
        decode_receipt(&receipt_bytes()).unwrap()
    }

    #[test]
    fn a_stable_stopped_trace_is_complete_and_never_a_quantity() {
        let mut bytes = blank_trace(2);
        set_row(&mut bytes, 0, 2, 1, 0xdead_beef_0000_0001);
        let view = TraceView::parse(&bytes).unwrap();
        let json = project_trace(&view, &view, &receipt());
        assert_eq!(json["complete"], true);
        assert_eq!(json["quantity_accepted"], false);
        assert_eq!(json["accepted"], false);
        let row = &json["rows"][0];
        assert_eq!(row["state"], "completed");
        assert_eq!(row["original_al"], 1);
        assert_eq!(row["caller_rbp"], "0xaaaabbbb");
        assert_eq!(row["caller_rdi"], "0xccccdddd");
        assert_eq!(row["call"], "apply");
    }

    #[test]
    fn unstable_unpublished_and_active_rows_are_unknown_not_zero() {
        let mut a = blank_trace(0);
        set_row(&mut a, 0, 1, 2, 0x77);
        let mut b = a.clone();
        set_row(&mut b, 1, 2, 1, 5);
        let (a, b) = (TraceView::parse(&a).unwrap(), TraceView::parse(&b).unwrap());
        let json = project_trace(&a, &b, &receipt());
        assert_eq!(json["complete"], false);
        assert_eq!(json["rows"][0]["state"], "incomplete");
        assert_eq!(json["rows"][0]["original_rax"], Value::Null);
        assert_eq!(json["rows"][1]["state"], "unknown_unstable");
        assert_eq!(json["rows"][1]["original_rax"], Value::Null);
        // A stopped trace with a row that was reserved and never published.
        let mut c = blank_trace(2);
        set_row(&mut c, 0, 0, 1, 0);
        let c = TraceView::parse(&c).unwrap();
        let json = project_trace(&c, &c, &receipt());
        assert_eq!(json["rows"][0]["state"], "unknown_unpublished");
        assert_eq!(json["complete"], false);
        // In flight keeps a stopped trace incomplete.
        let mut d = blank_trace(2);
        d[H_IN_FLIGHT] = 1;
        let d = TraceView::parse(&d).unwrap();
        assert_eq!(project_trace(&d, &d, &receipt())["complete"], false);
    }

    #[test]
    fn trace_parse_refuses_wrong_magic_version_and_length() {
        let good = blank_trace(0);
        assert!(TraceView::parse(&good).is_ok());
        let mut magic = good.clone();
        magic[0] ^= 1;
        assert!(TraceView::parse(&magic).is_err());
        let mut version = good.clone();
        version[4] = 3;
        assert!(TraceView::parse(&version).is_err());
        assert!(TraceView::parse(&good[..TRACE_SIZE - 1]).is_err());
        let mut overflow = good;
        overflow[H_NEXT..H_NEXT + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(TraceView::parse(&overflow).is_err());
        overflow[H_NEXT..H_NEXT + 4].copy_from_slice(&(CAPACITY as u32 + 1).to_le_bytes());
        assert!(TraceView::parse(&overflow).is_err());
        overflow[H_NEXT..H_NEXT + 4].copy_from_slice(&(CAPACITY as u32).to_le_bytes());
        assert_eq!(TraceView::parse(&overflow).unwrap().rows, CAPACITY);
    }

    #[test]
    fn unknown_status_values_are_named_not_mapped_to_ok() {
        assert_eq!(status_name(0), "ok");
        assert_eq!(status_name(8), "not_wired");
        assert_eq!(status_name(1234), "unknown_status_1234");
        assert!(lower_hex_sha(&"a".repeat(64)));
        assert!(!lower_hex_sha(&"A".repeat(64)));
        assert!(!lower_hex_sha("abc"));
    }

    /// Smallest DLL image with one executable section and the given exports.
    pub(crate) fn synthetic_dll(names: &[&str], forward_first: bool) -> Vec<u8> {
        let mut f = vec![0u8; 0x600];
        f[0..2].copy_from_slice(b"MZ");
        f[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        let pe = 0x80;
        f[pe..pe + 4].copy_from_slice(b"PE\0\0");
        f[pe + 4..pe + 6].copy_from_slice(&0x8664u16.to_le_bytes());
        f[pe + 6..pe + 8].copy_from_slice(&1u16.to_le_bytes());
        f[pe + 20..pe + 22].copy_from_slice(&240u16.to_le_bytes());
        f[pe + 22..pe + 24].copy_from_slice(&0x2022u16.to_le_bytes());
        let opt = pe + 24;
        f[opt..opt + 2].copy_from_slice(&0x20bu16.to_le_bytes());
        f[opt + 24..opt + 32].copy_from_slice(&0x1_8000_0000u64.to_le_bytes());
        f[opt + 56..opt + 60].copy_from_slice(&0x3000u32.to_le_bytes());
        f[opt + 60..opt + 64].copy_from_slice(&0x200u32.to_le_bytes());
        f[opt + 112..opt + 116].copy_from_slice(&0x1100u32.to_le_bytes());
        f[opt + 116..opt + 120].copy_from_slice(&0x100u32.to_le_bytes());
        let sec = opt + 240;
        f[sec..sec + 6].copy_from_slice(b".text\0");
        f[sec + 8..sec + 12].copy_from_slice(&0x1000u32.to_le_bytes());
        f[sec + 12..sec + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        f[sec + 16..sec + 20].copy_from_slice(&0x400u32.to_le_bytes());
        f[sec + 20..sec + 24].copy_from_slice(&0x200u32.to_le_bytes());
        f[sec + 36..sec + 40].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        let raw = |rva: u32| 0x200 + (rva - 0x1000) as usize;
        let d = raw(0x1100);
        let n = names.len() as u32;
        f[d + 20..d + 24].copy_from_slice(&n.to_le_bytes());
        f[d + 24..d + 28].copy_from_slice(&n.to_le_bytes());
        f[d + 28..d + 32].copy_from_slice(&0x1130u32.to_le_bytes());
        f[d + 32..d + 36].copy_from_slice(&0x1150u32.to_le_bytes());
        f[d + 36..d + 40].copy_from_slice(&0x1170u32.to_le_bytes());
        let mut name_at = 0x1180u32;
        for (i, name) in names.iter().enumerate() {
            let i = i as u32;
            let func = if forward_first && i == 0 {
                0x1110
            } else {
                0x1010 + 0x10 * i
            };
            f[raw(0x1130 + 4 * i)..][..4].copy_from_slice(&func.to_le_bytes());
            f[raw(0x1150 + 4 * i)..][..4].copy_from_slice(&name_at.to_le_bytes());
            f[raw(0x1170 + 2 * i)..][..2].copy_from_slice(&(i as u16).to_le_bytes());
            f[raw(name_at)..][..name.len()].copy_from_slice(name.as_bytes());
            name_at += name.len() as u32 + 1;
        }
        f
    }

    #[test]
    fn export_parsing_finds_exactly_the_four_executable_exports() {
        let dll = synthetic_dll(&EXPORTS, false);
        let image = parse_pe(&dll).unwrap();
        let rvas = runtime_exports(&image).unwrap();
        assert_eq!(rvas, [0x1010, 0x1020, 0x1030, 0x1040]);
        assert_eq!(image.size_of_image, 0x3000);
        assert!(runtime_exports(&parse_pe(&synthetic_dll(&EXPORTS[..3], false)).unwrap()).is_err());
        let dup = [EXPORTS[0], EXPORTS[0], EXPORTS[1], EXPORTS[2]];
        assert!(runtime_exports(&parse_pe(&synthetic_dll(&dup, false)).unwrap()).is_err());
        assert!(parse_pe(&synthetic_dll(&EXPORTS, true)).is_err());
        let mut exe = dll.clone();
        exe[0x80 + 22] = 0x22;
        exe[0x80 + 23] = 0x00;
        assert!(parse_pe(&exe).is_err());
        let mut x86 = dll.clone();
        x86[0x84] = 0x4c;
        assert!(parse_pe(&x86).is_err());
        assert!(parse_pe(&dll[..0x300]).is_err());
        assert!(parse_pe(b"MZ").is_err());
    }

    #[test]
    fn tables_that_straddle_a_section_end_or_overlap_are_refused() {
        let good = synthetic_dll(&EXPORTS, false);
        assert!(parse_pe(&good).is_ok());
        let dir = 0x200 + 0x100; // export directory at RVA 0x1100
        let put = |bytes: &mut Vec<u8>, at: usize, value: u32| {
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        // each table pointer one byte short of the section end (0x1400)
        for (field, rva) in [(32usize, 0x13feu32), (36, 0x13ff), (28, 0x13fd)] {
            let mut bytes = good.clone();
            put(&mut bytes, dir + field, rva);
            assert!(parse_pe(&bytes).is_err(), "table field {field} at {rva:#x}");
        }
        // export directory whose 40 bytes cross the section end
        let mut bytes = good.clone();
        put(&mut bytes, 0x80 + 24 + 112, 0x13f0);
        assert!(parse_pe(&bytes).is_err());
        // a name that starts at the last byte must not read into the next section
        let mut bytes = good.clone();
        let names = 0x200 + 0x150; // name pointer table at RVA 0x1150
        put(&mut bytes, names, 0x13ff);
        bytes[0x200 + 0x3ff] = b'A';
        assert!(parse_pe(&bytes).is_err(), "unterminated inside the section");
        // two sections sharing the same raw data, in either order
        let mut bytes = good.clone();
        let first = 0x80 + 24 + 240;
        let copy = bytes[first..first + 40].to_vec();
        bytes[first + 40..first + 80].copy_from_slice(&copy);
        bytes[0x80 + 6] = 2;
        assert!(parse_pe(&bytes).is_err(), "same raw and virtual range");
        let mut reversed = good.clone();
        reversed[first + 40..first + 80].copy_from_slice(&copy);
        reversed[first + 40 + 12..first + 40 + 16].copy_from_slice(&0x2000u32.to_le_bytes());
        reversed[0x80 + 6] = 2;
        assert!(
            parse_pe(&reversed).is_err(),
            "same raw data under another address"
        );
        // bytes_at: zero width, width past the section, and a normal read
        let image = parse_pe(&good).unwrap();
        assert!(image.bytes_at(&good, 0x1010, 0).is_none());
        assert!(image.bytes_at(&good, 0x13ff, 2).is_none());
        assert!(image.bytes_at(&good, u32::MAX, 4).is_none());
        assert_eq!(image.bytes_at(&good, 0x1010, 32).map(<[u8]>::len), Some(32));
    }

    #[test]
    fn header_comparison_masks_only_the_image_base() {
        let dll = synthetic_dll(&EXPORTS, false);
        let image = parse_pe(&dll).unwrap();
        let mut remote = dll.clone();
        remote[image.image_base_offset..image.image_base_offset + 8]
            .copy_from_slice(&0x7ff6_0000_0000u64.to_le_bytes());
        assert!(header_matches(&dll, &remote, &image));
        remote[image.image_base_offset + 8] ^= 1;
        assert!(!header_matches(&dll, &remote, &image));
        assert!(!header_matches(&dll[..0x100], &dll, &image));
    }

    fn fact(path: &str, base: usize, size: usize) -> ModuleFact {
        ModuleFact {
            path: path.into(),
            base,
            size,
        }
    }

    #[test]
    fn a_forwarded_function_is_proven_by_its_real_owner_not_the_lookup_module() {
        let kernelbase = fact(
            r"C:\Windows\System32\KERNELBASE.dll",
            0x7ff8_0000_0000,
            0x30_0000,
        );
        let kernel32 = fact(
            r"C:\Windows\System32\kernel32.dll",
            0x7ff8_1000_0000,
            0x10_0000,
        );
        let remote = vec![kernel32.clone(), kernelbase.clone()];
        let function = kernelbase.base + 0x1234;
        // the real owner (KernelBase) matches; kernel32 alone does not prove it
        assert_eq!(
            function_owner_in_remote(function, &kernelbase, &remote),
            Ok(1)
        );
        assert!(function_owner_in_remote(function, &kernel32, &remote).is_err());
        assert!(
            function_owner_in_remote(function, &kernelbase, std::slice::from_ref(&kernel32))
                .is_err()
        );
        // same path, different base or size remotely
        let moved = vec![fact(
            r"\\?\c:\windows\system32\kernelbase.DLL",
            kernelbase.base + 0x10000,
            kernelbase.size,
        )];
        assert!(function_owner_in_remote(function, &kernelbase, &moved).is_err());
        let resized = vec![fact(&kernelbase.path, kernelbase.base, kernelbase.size + 1)];
        assert!(function_owner_in_remote(function, &kernelbase, &resized).is_err());
        // case and verbatim prefix are not differences
        let same = vec![fact(
            r"\\?\c:\WINDOWS\system32\kernelbase.dll",
            kernelbase.base,
            kernelbase.size,
        )];
        assert_eq!(
            function_owner_in_remote(function, &kernelbase, &same),
            Ok(0)
        );
        // duplicate remote entries and an address outside the owner
        let twice = vec![kernelbase.clone(), kernelbase.clone()];
        assert!(function_owner_in_remote(function, &kernelbase, &twice).is_err());
        assert!(
            function_owner_in_remote(kernelbase.base + kernelbase.size, &kernelbase, &remote)
                .is_err()
        );
        assert!(function_owner_in_remote(kernelbase.base - 1, &kernelbase, &remote).is_err());
    }

    #[test]
    fn a_zero_load_result_is_unknown_unless_a_module_proves_it() {
        let m = fact("x.dll", 0x1_0000_0000, 0x3000);
        // low half of the base is 0: legitimate, decided by the module
        assert_eq!(
            judge_load(0, 0x3000, std::slice::from_ref(&m)),
            LoadVerdict::Loaded(0x1_0000_0000)
        );
        // 0 with no module: not "failed", unknown
        assert!(matches!(
            judge_load(0, 0x3000, &[]),
            LoadVerdict::Unknown(_)
        ));
        assert!(matches!(
            judge_load(5, 0x3000, &[]),
            LoadVerdict::Unknown(_)
        ));
        // wrong low half, wrong size, duplicates
        assert!(matches!(
            judge_load(7, 0x3000, std::slice::from_ref(&m)),
            LoadVerdict::Unknown(_)
        ));
        assert!(matches!(
            judge_load(0, 0x4000, std::slice::from_ref(&m)),
            LoadVerdict::Unknown(_)
        ));
        assert!(matches!(
            judge_load(0, 0x3000, &[m.clone(), m]),
            LoadVerdict::Unknown(_)
        ));
    }
}
