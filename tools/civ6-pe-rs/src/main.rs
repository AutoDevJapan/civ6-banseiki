//! Read-only PE64 probe for unwind ranges and relative branches.
//! Byte-pattern hits are candidates until confirmed with a real disassembler.
use std::env;
use std::fs;
use std::path::Path;

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

fn main() {
    let mut args = env::args_os().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: civ6-pe <read-only DLL path> <hex RVA> [hex RVA ...]");
        std::process::exit(2);
    };
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
}
