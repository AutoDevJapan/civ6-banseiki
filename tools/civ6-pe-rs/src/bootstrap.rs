//! Disk-only bootstrap investigation. Hits are NOT approved patch locations.
use super::*;

fn cstr(bytes: &[u8], offset: usize) -> Result<&str, String> {
    let tail = bytes.get(offset..).ok_or("string outside file")?;
    let end = tail
        .iter()
        .position(|b| *b == 0)
        .ok_or("unterminated string")?;
    std::str::from_utf8(&tail[..end]).map_err(|e| e.to_string())
}

fn refs(pe: &Pe<'_>, target: u32) -> Result<(), String> {
    println!("target RVA 0x{target:x}");
    if let Some(range) = pe.unwind_range(target) {
        println!("  pdata [0x{:x},0x{:x})", range.begin, range.end);
    }
    for section in &pe.sections {
        let data = at(pe.bytes, section.file_offset, section.file_size)?;
        if section.name.starts_with(b".text") {
            for i in 0..data.len().saturating_sub(5) {
                let code = &data[i..];
                let displacement = if code.len() >= 7
                    && matches!(code[0], 0x48 | 0x4c)
                    && matches!(code[1], 0x8d | 0x8b | 0x89)
                    && code[2] & 0xc7 == 5
                {
                    Some((
                        7,
                        i32::from_le_bytes(code[3..7].try_into().unwrap()),
                        "rip mov/lea",
                    ))
                } else if code[0] == 0xff && matches!(code[1], 0x15 | 0x25) {
                    Some((
                        6,
                        i32::from_le_bytes(code[2..6].try_into().unwrap()),
                        "indirect call/jump",
                    ))
                } else {
                    None
                };
                if let Some((len, delta, kind)) = displacement {
                    let source = section.rva as i64 + i as i64;
                    if source + len + i64::from(delta) == i64::from(target) {
                        print!("  {kind} candidate RVA 0x{source:x}");
                        if let Some(range) = pe.unwind_range(source as u32) {
                            print!(" pdata [0x{:x},0x{:x})", range.begin, range.end);
                        }
                        println!();
                    }
                }
            }
        }
        for (i, code) in data.windows(8).enumerate() {
            if u64::from_le_bytes(code.try_into().unwrap()) == pe.image_base + u64::from(target) {
                println!(
                    "  absolute VA candidate RVA 0x{:x}",
                    section.rva as usize + i
                );
            }
        }
    }
    for (source, op) in pe.branches_to(target)? {
        print!("  {op:02X} branch candidate RVA 0x{source:x}");
        if let Some(range) = pe.unwind_range(source) {
            print!(" pdata [0x{:x},0x{:x})", range.begin, range.end);
        }
        println!();
    }
    Ok(())
}

pub(super) fn inspect(path: &Path, query: &str) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let pe = Pe::parse(&bytes)?;
    println!(
        "disk-only; image base 0x{:x}; bytes {}",
        pe.image_base,
        bytes.len()
    );
    println!("Byte-pattern references require disassembler confirmation; no process access or runtime acceptance.");
    if query.starts_with("0x") {
        return refs(&pe, parse_rva(query)?);
    }
    if query.is_empty() || !query.is_ascii() || query.contains('\0') {
        return Err("nonempty ASCII query required".into());
    }
    for target in pe.ascii_rvas(query.as_bytes())? {
        refs(&pe, target)?;
    }
    let optional = u32_at(&bytes, 0x3c)? as usize + 24;
    let directory_count = u32_at(&bytes, optional + 108)?;
    if directory_count < 2 {
        return Ok(());
    }
    let imports = u32_at(&bytes, optional + 120)?;
    if imports == 0 {
        return Ok(());
    }
    let mut descriptor = pe.offset(imports).ok_or("imports outside file")?;
    loop {
        let original = u32_at(&bytes, descriptor)?;
        let name = u32_at(&bytes, descriptor + 12)?;
        let first = u32_at(&bytes, descriptor + 16)?;
        if original == 0 && name == 0 && first == 0 {
            break;
        }
        let module = cstr(&bytes, pe.offset(name).ok_or("module outside file")?)?;
        let thunk = if original == 0 { first } else { original };
        let mut offset = pe.offset(thunk).ok_or("thunk outside file")?;
        let mut index = 0u32;
        loop {
            let value = u64_at(&bytes, offset)?;
            if value == 0 {
                break;
            }
            if value >> 63 == 0 {
                let rva = u32::try_from(value).map_err(|_| "invalid name RVA")?;
                let symbol = cstr(
                    &bytes,
                    pe.offset(rva).ok_or("import name outside file")? + 2,
                )?;
                if symbol.contains(query) {
                    let slot = first
                        .checked_add(index.checked_mul(8).ok_or("IAT overflow")?)
                        .ok_or("IAT overflow")?;
                    println!("import {module}!{symbol}; IAT RVA 0x{slot:x}");
                    refs(&pe, slot)?;
                }
            }
            offset += 8;
            index += 1;
        }
        descriptor += 20;
    }
    Ok(())
}
