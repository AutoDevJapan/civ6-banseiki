//! Read-only PE64 probe for unwind ranges and relative branches.
//! Byte-pattern hits are candidates until confirmed with a real disassembler.
use std::env;
use std::fs;
use std::path::Path;

mod bootstrap;
mod bootstrap_audit;

#[derive(Clone, Copy)]
struct Section {
    name: [u8; 8],
    rva: u32,
    virtual_size: u32,
    file_offset: usize,
    file_size: usize,
}

#[derive(Clone, Copy)]
struct UnwindRange {
    begin: u32,
    end: u32,
}

struct Pe<'a> {
    bytes: &'a [u8],
    sections: Vec<Section>,
    unwind_ranges: Vec<UnwindRange>,
    image_base: u64,
}

fn at(bytes: &[u8], offset: usize, count: usize) -> Result<&[u8], String> {
    bytes
        .get(offset..offset.checked_add(count).ok_or("offset overflow")?)
        .ok_or_else(|| format!("file is truncated at 0x{offset:x}"))
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        at(bytes, offset, 2)?.try_into().unwrap(),
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        at(bytes, offset, 4)?.try_into().unwrap(),
    ))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        at(bytes, offset, 8)?.try_into().unwrap(),
    ))
}

impl<'a> Pe<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if at(bytes, 0, 2)? != b"MZ" {
            return Err("not an MZ executable".into());
        }
        let pe_offset = u32_at(bytes, 0x3c)? as usize;
        if at(bytes, pe_offset, 4)? != b"PE\0\0" {
            return Err("not a PE executable".into());
        }
        let coff = pe_offset + 4;
        if u16_at(bytes, coff)? != 0x8664 {
            return Err("not x86-64 PE".into());
        }
        let section_count = u16_at(bytes, coff + 2)? as usize;
        let optional_size = u16_at(bytes, coff + 16)? as usize;
        let optional = coff + 20;
        if optional_size < 0x70 || u16_at(bytes, optional)? != 0x20b {
            return Err("not PE32+".into());
        }
        let image_base = u64_at(bytes, optional + 24)?;
        let section_table = optional
            .checked_add(optional_size)
            .ok_or("section overflow")?;
        let mut sections = Vec::with_capacity(section_count);
        for i in 0..section_count {
            let p = section_table
                .checked_add(i.checked_mul(40).ok_or("section overflow")?)
                .ok_or("section overflow")?;
            let name = at(bytes, p, 8)?.try_into().unwrap();
            let virtual_size = u32_at(bytes, p + 8)?;
            let rva = u32_at(bytes, p + 12)?;
            let file_size = u32_at(bytes, p + 16)? as usize;
            let file_offset = u32_at(bytes, p + 20)? as usize;
            at(bytes, file_offset, file_size)?;
            sections.push(Section {
                name,
                rva,
                virtual_size,
                file_offset,
                file_size,
            });
        }
        let mut pe = Self {
            bytes,
            sections,
            unwind_ranges: Vec::new(),
            image_base,
        };
        let pdata = pe.section(b".pdata")?;
        let data = at(bytes, pdata.file_offset, pdata.file_size)?;
        for entry in data.chunks_exact(12) {
            let begin = u32::from_le_bytes(entry[0..4].try_into().unwrap());
            let end = u32::from_le_bytes(entry[4..8].try_into().unwrap());
            if begin < end {
                pe.unwind_ranges.push(UnwindRange { begin, end });
            }
        }
        pe.unwind_ranges.sort_by_key(|range| range.begin);
        Ok(pe)
    }

    fn section(&self, name: &[u8]) -> Result<Section, String> {
        self.sections
            .iter()
            .copied()
            .find(|s| s.name.split(|b| *b == 0).next() == Some(name))
            .ok_or_else(|| format!("missing section {}", String::from_utf8_lossy(name)))
    }

    fn offset(&self, rva: u32) -> Option<usize> {
        self.sections.iter().find_map(|s| {
            let relative = rva.checked_sub(s.rva)? as usize;
            (relative < s.file_size && relative < s.virtual_size as usize)
                .then_some(s.file_offset + relative)
        })
    }

    fn unwind_range(&self, rva: u32) -> Option<UnwindRange> {
        let i = self
            .unwind_ranges
            .partition_point(|range| range.begin <= rva);
        i.checked_sub(1)
            .and_then(|i| self.unwind_ranges.get(i).copied())
            .filter(|range| rva < range.end)
    }

    fn branches_to(&self, target: u32) -> Result<Vec<(u32, u8)>, String> {
        let text = self.section(b".text")?;
        let data = at(self.bytes, text.file_offset, text.file_size)?;
        let mut hits = Vec::new();
        for (i, code) in data.windows(5).enumerate() {
            if code[0] != 0xe8 && code[0] != 0xe9 {
                continue;
            }
            let displacement = i32::from_le_bytes(code[1..5].try_into().unwrap()) as i64;
            let source = text.rva as i64 + i as i64;
            if source + 5 + displacement == target as i64 {
                hits.push((source as u32, code[0]));
            }
        }
        Ok(hits)
    }

    fn ascii_rvas(&self, needle: &[u8]) -> Result<Vec<u32>, String> {
        if needle.is_empty() || !needle.is_ascii() || needle.contains(&0) {
            return Err("search text must be nonempty ASCII without NUL".into());
        }
        let mut hits = Vec::new();
        for section in &self.sections {
            let data = at(self.bytes, section.file_offset, section.file_size)?;
            for (i, candidate) in data.windows(needle.len() + 1).enumerate() {
                if (i == 0 || data[i - 1] == 0)
                    && candidate[..needle.len()] == *needle
                    && candidate[needle.len()] == 0
                {
                    hits.push(section.rva + i as u32);
                }
            }
        }
        Ok(hits)
    }

    fn rip_lea_refs(&self, target: u32) -> Result<Vec<u32>, String> {
        let text = self.section(b".text")?;
        let data = at(self.bytes, text.file_offset, text.file_size)?;
        let mut hits = Vec::new();
        for (i, code) in data.windows(7).enumerate() {
            if !matches!(code[0], 0x48 | 0x4c) || code[1] != 0x8d || code[2] & 0xc7 != 0x05 {
                continue;
            }
            let displacement = i32::from_le_bytes(code[3..7].try_into().unwrap()) as i64;
            let source = text.rva as i64 + i as i64;
            if source + 7 + displacement == target as i64 {
                hits.push(source as u32);
            }
        }
        Ok(hits)
    }
}

fn parse_rva(arg: &str) -> Result<u32, String> {
    let hex = arg.strip_prefix("0x").unwrap_or(arg);
    u32::from_str_radix(hex, 16).map_err(|e| format!("invalid hexadecimal RVA {arg}: {e}"))
}

fn run(path: &Path, targets: &[String]) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pe = Pe::parse(&bytes)?;
    println!(
        "image base 0x{:x}; file bytes {}",
        pe.image_base,
        bytes.len()
    );
    println!("E8/E9 matches are byte-pattern candidates; verify instruction boundaries with a disassembler. .pdata ranges may be function fragments.");
    for target in targets {
        let rva = parse_rva(target)?;
        let offset = pe
            .offset(rva)
            .ok_or_else(|| format!("RVA 0x{rva:x} has no file bytes"))?;
        let sample = at(&bytes, offset, 12)?;
        print!("target 0x{rva:x} file 0x{offset:x} bytes");
        for byte in sample {
            print!(" {byte:02x}");
        }
        println!();
        if let Some(f) = pe.unwind_range(rva) {
            println!("  .pdata range [0x{:x}, 0x{:x})", f.begin, f.end);
        } else {
            println!("  no .pdata range");
        }
        for (source, opcode) in pe.branches_to(rva)? {
            let mnemonic = if opcode == 0xe8 { "call" } else { "jump" };
            if let Some(f) = pe.unwind_range(source) {
                println!(
                    "  {mnemonic} candidate 0x{source:x} in [0x{:x}, 0x{:x})",
                    f.begin, f.end
                );
            } else {
                println!("  {mnemonic} candidate 0x{source:x} (no .pdata range)");
            }
        }
    }
    Ok(())
}

fn run_ascii(path: &Path, needle: &str) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pe = Pe::parse(&bytes)?;
    println!("image base 0x{:x}; ASCII text {needle:?}", pe.image_base);
    println!("RIP LEA hits are byte-pattern candidates; verify instruction boundaries with a disassembler.");
    for rva in pe.ascii_rvas(needle.as_bytes())? {
        let offset = pe.offset(rva).ok_or("matched string has no file bytes")?;
        println!("string RVA 0x{rva:x}, file 0x{offset:x}");
        for source in pe.rip_lea_refs(rva)? {
            println!("  LEA candidate RVA 0x{source:x}");
        }
    }
    Ok(())
}

fn main() {
    let mut args = env::args_os().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: civ6-pe <read-only DLL path> <hex RVA>... | civ6-pe --ascii-xrefs <read-only DLL path> <ASCII text>");
        std::process::exit(2);
    };
    if path == "--bootstrap-audit" {
        let values: Vec<_> = args.collect();
        if values.len() != 2 {
            eprintln!("usage: civ6-pe --bootstrap-audit <CivilizationVI.exe> <GameCore_XP2_FinalRelease.dll>");
            std::process::exit(2);
        }
        if let Err(error) = bootstrap_audit::audit(Path::new(&values[0]), Path::new(&values[1])) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if path == "--bootstrap" {
        let values: Vec<_> = args.collect();
        if values.len() != 2 {
            eprintln!("usage: civ6-pe --bootstrap <read-only PE path> <ASCII name or 0xRVA>");
            std::process::exit(2);
        }
        if let Err(error) = bootstrap::inspect(Path::new(&values[0]), &values[1].to_string_lossy())
        {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if path == "--ascii-xrefs" {
        let Some(dll) = args.next() else {
            eprintln!("a DLL path is required");
            std::process::exit(2);
        };
        let Some(needle) = args.next() else {
            eprintln!("ASCII text is required");
            std::process::exit(2);
        };
        if args.next().is_some() {
            eprintln!("exactly one ASCII text argument is required");
            std::process::exit(2);
        }
        if let Err(error) = run_ascii(Path::new(&dll), &needle.to_string_lossy()) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let targets: Vec<_> = args.map(|v| v.to_string_lossy().into_owned()).collect();
    if targets.is_empty() {
        eprintln!("at least one RVA is required");
        std::process::exit(2);
    }
    if let Err(error) = run(Path::new(&path), &targets) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_branch_target_handles_negative_displacement() {
        let mut bytes = vec![0; 0x500];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x80_u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        bytes[0x84..0x86].copy_from_slice(&0x8664_u16.to_le_bytes());
        bytes[0x86..0x88].copy_from_slice(&2_u16.to_le_bytes());
        bytes[0x94..0x96].copy_from_slice(&0xf0_u16.to_le_bytes());
        bytes[0x98..0x9a].copy_from_slice(&0x20b_u16.to_le_bytes());
        bytes[0xb0..0xb8].copy_from_slice(&0x180000000_u64.to_le_bytes());
        let sections = 0x188;
        bytes[sections..sections + 5].copy_from_slice(b".text");
        bytes[sections + 8..sections + 12].copy_from_slice(&0x100_u32.to_le_bytes());
        bytes[sections + 12..sections + 16].copy_from_slice(&0x1000_u32.to_le_bytes());
        bytes[sections + 16..sections + 20].copy_from_slice(&0x100_u32.to_le_bytes());
        bytes[sections + 20..sections + 24].copy_from_slice(&0x200_u32.to_le_bytes());
        let pdata = sections + 40;
        bytes[pdata..pdata + 6].copy_from_slice(b".pdata");
        bytes[pdata + 8..pdata + 12].copy_from_slice(&12_u32.to_le_bytes());
        bytes[pdata + 12..pdata + 16].copy_from_slice(&0x2000_u32.to_le_bytes());
        bytes[pdata + 16..pdata + 20].copy_from_slice(&12_u32.to_le_bytes());
        bytes[pdata + 20..pdata + 24].copy_from_slice(&0x400_u32.to_le_bytes());
        bytes[0x400..0x404].copy_from_slice(&0x1000_u32.to_le_bytes());
        bytes[0x404..0x408].copy_from_slice(&0x1050_u32.to_le_bytes());
        bytes[0x210..0x215].copy_from_slice(&[0xe8, 0xeb, 0xff, 0xff, 0xff]);
        let pe = Pe::parse(&bytes).unwrap();
        assert_eq!(pe.branches_to(0x1000).unwrap(), vec![(0x1010, 0xe8)]);
        assert_eq!(pe.unwind_range(0x1010).unwrap().begin, 0x1000);
        assert_eq!(pe.offset(0x1010), Some(0x210));
    }

    #[test]
    fn ascii_xrefs_resolve_section_rva_and_rip_relative_lea() {
        let mut bytes = vec![0; 0x600];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x80_u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        bytes[0x84..0x86].copy_from_slice(&0x8664_u16.to_le_bytes());
        bytes[0x86..0x88].copy_from_slice(&3_u16.to_le_bytes());
        bytes[0x94..0x96].copy_from_slice(&0xf0_u16.to_le_bytes());
        bytes[0x98..0x9a].copy_from_slice(&0x20b_u16.to_le_bytes());
        bytes[0xb0..0xb8].copy_from_slice(&0x180000000_u64.to_le_bytes());
        let sections = 0x188;
        bytes[sections..sections + 5].copy_from_slice(b".text");
        bytes[sections + 8..sections + 12].copy_from_slice(&0x100_u32.to_le_bytes());
        bytes[sections + 12..sections + 16].copy_from_slice(&0x1000_u32.to_le_bytes());
        bytes[sections + 16..sections + 20].copy_from_slice(&0x100_u32.to_le_bytes());
        bytes[sections + 20..sections + 24].copy_from_slice(&0x200_u32.to_le_bytes());
        let pdata = sections + 40;
        bytes[pdata..pdata + 6].copy_from_slice(b".pdata");
        bytes[pdata + 8..pdata + 12].copy_from_slice(&12_u32.to_le_bytes());
        bytes[pdata + 12..pdata + 16].copy_from_slice(&0x2000_u32.to_le_bytes());
        bytes[pdata + 16..pdata + 20].copy_from_slice(&12_u32.to_le_bytes());
        bytes[pdata + 20..pdata + 24].copy_from_slice(&0x400_u32.to_le_bytes());
        let rdata = pdata + 40;
        bytes[rdata..rdata + 6].copy_from_slice(b".rdata");
        bytes[rdata + 8..rdata + 12].copy_from_slice(&0x20_u32.to_le_bytes());
        bytes[rdata + 12..rdata + 16].copy_from_slice(&0x3000_u32.to_le_bytes());
        bytes[rdata + 16..rdata + 20].copy_from_slice(&0x20_u32.to_le_bytes());
        bytes[rdata + 20..rdata + 24].copy_from_slice(&0x480_u32.to_le_bytes());
        bytes[0x220..0x227].copy_from_slice(&[0x4c, 0x8d, 0x0d, 0xd9, 0x1f, 0, 0]);
        bytes[0x480..0x489].copy_from_slice(b"GetThing\0");
        let pe = Pe::parse(&bytes).unwrap();
        assert_eq!(pe.ascii_rvas(b"GetThing").unwrap(), vec![0x3000]);
        assert_eq!(pe.ascii_rvas(b"Thing").unwrap(), Vec::<u32>::new());
        assert_eq!(pe.rip_lea_refs(0x3000).unwrap(), vec![0x1020]);
        assert_eq!(pe.offset(0x3000), Some(0x480));
    }
}
