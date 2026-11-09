//! Strict file-buffer PE mapping for the session-boundary audit.
use std::collections::BTreeSet;

pub(super) fn at(b: &[u8], start: usize, len: usize) -> Result<&[u8], String> {
    b.get(start..start.checked_add(len).ok_or("file span overflow")?)
        .ok_or_else(|| "truncated file span".into())
}
pub(super) fn u16_at(b: &[u8], p: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(at(b, p, 2)?.try_into().unwrap()))
}
pub(super) fn u32_at(b: &[u8], p: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(at(b, p, 4)?.try_into().unwrap()))
}
pub(super) fn u64_at(b: &[u8], p: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(at(b, p, 8)?.try_into().unwrap()))
}
struct Section {
    rva: u32,
    raw: usize,
    len: u32,
    virtual_len: u32,
    flags: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Function {
    pub begin: u32,
    pub end: u32,
    pub unwind: u32,
}
pub(super) struct Pe<'a> {
    bytes: &'a [u8],
    pub base: u64,
    sections: Vec<Section>,
    pub functions: Vec<Function>,
    relocations: BTreeSet<u32>,
}
impl<'a> Pe<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        Self::parse_image(bytes, true)
    }
    pub fn parse_host(bytes: &'a [u8]) -> Result<Self, String> {
        Self::parse_image(bytes, false)
    }
    fn parse_image(bytes: &'a [u8], dll: bool) -> Result<Self, String> {
        if at(bytes, 0, 2)? != b"MZ" {
            return Err("operation PE MZ signature".into());
        }
        let nt = u32_at(bytes, 0x3c)? as usize;
        if at(bytes, nt, 4)? != b"PE\0\0" {
            return Err("operation PE signature".into());
        }
        let coff = nt.checked_add(4).ok_or("PE header overflow")?;
        let opt = coff.checked_add(20).ok_or("PE header overflow")?;
        let count = u16_at(bytes, coff + 2)? as usize;
        let size = u16_at(bytes, coff + 16)? as usize;
        if u16_at(bytes, coff)? != 0x8664
            || u16_at(bytes, coff + 18)? & 0x2002 != if dll { 0x2002 } else { 2 }
            || !(1..=96).contains(&count)
            || size < 160
            || u16_at(bytes, opt)? != 0x20b
            || u32_at(bytes, opt + 108)? < 6
        {
            return Err("session PE64 image headers".into());
        }
        at(bytes, opt, size)?;
        let table = opt.checked_add(size).ok_or("PE table overflow")?;
        let mut sections = Vec::new();
        for i in 0..count {
            let p = table + i * 40;
            at(bytes, p, 40)?;
            let raw_len = u32_at(bytes, p + 16)?;
            let s = Section {
                rva: u32_at(bytes, p + 12)?,
                raw: u32_at(bytes, p + 20)? as usize,
                len: u32_at(bytes, p + 8)?.min(raw_len),
                virtual_len: u32_at(bytes, p + 8)?,
                flags: u32_at(bytes, p + 36)?,
            };
            s.rva.checked_add(s.len).ok_or("section RVA overflow")?;
            s.rva
                .checked_add(s.virtual_len)
                .ok_or("virtual section RVA overflow")?;
            at(bytes, s.raw, raw_len as usize)?;
            sections.push(s);
        }
        let mut pe = Self {
            bytes,
            base: u64_at(bytes, opt + 24)?,
            sections,
            functions: Vec::new(),
            relocations: BTreeSet::new(),
        };
        let prva = u32_at(bytes, opt + 112 + 3 * 8)?;
        let plen = u32_at(bytes, opt + 116 + 3 * 8)? as usize;
        if plen == 0 || !plen.is_multiple_of(12) || plen / 12 > 131072 {
            return Err("operation exception directory size".into());
        }
        for row in pe.span(prva, plen, false)?.chunks_exact(12) {
            let f = Function {
                begin: u32_at(row, 0)?,
                end: u32_at(row, 4)?,
                unwind: u32_at(row, 8)?,
            };
            if f.begin >= f.end || f.unwind & 3 != 0 {
                return Err("operation pdata range/alignment".into());
            }
            pe.span(f.begin, (f.end - f.begin) as usize, true)?;
            pe.span(f.unwind, 4, false)?;
            if pe
                .functions
                .last()
                .is_some_and(|last: &Function| last.end > f.begin)
            {
                return Err("operation pdata order/overlap".into());
            }
            pe.functions.push(f);
        }
        let rrva = u32_at(bytes, opt + 112 + 5 * 8)?;
        let rlen = u32_at(bytes, opt + 116 + 5 * 8)? as usize;
        if (rrva == 0) != (rlen == 0) {
            return Err("session relocation address/size disagree".into());
        }
        if rlen == 0 {
            if dll {
                return Err("session Core DIR64 relocation directory absent".into());
            }
            return Ok(pe);
        }
        let reloc = pe.span(rrva, rlen, false)?;
        let mut p = 0;
        while p < reloc.len() {
            let page = u32_at(reloc, p)?;
            let size = u32_at(reloc, p + 4)? as usize;
            if size < 8 || !size.is_multiple_of(2) {
                return Err("operation relocation block size".into());
            }
            for word in at(reloc, p, size)?[8..].chunks_exact(2) {
                let value = u16::from_le_bytes(word.try_into().unwrap());
                if value >> 12 == 10 {
                    let rva = page
                        .checked_add(u32::from(value & 0xfff))
                        .ok_or("relocation overflow")?;
                    pe.span_any(rva, 8)?;
                    if !pe.relocations.insert(rva) {
                        return Err("duplicate operation DIR64 relocation".into());
                    }
                } else if value >> 12 != 0 {
                    return Err("operation relocation is not DIR64/ABSOLUTE".into());
                }
            }
            p = p.checked_add(size).ok_or("relocation length overflow")?;
        }
        Ok(pe)
    }
    fn section(&self, rva: u32, len: usize) -> Result<&Section, String> {
        let end = rva
            .checked_add(u32::try_from(len).map_err(|_| "span length overflow")?)
            .ok_or("RVA span overflow")?;
        let mut matches = self
            .sections
            .iter()
            .filter(|s| rva >= s.rva && end <= s.rva + s.len);
        let s = matches.next().ok_or("span outside backed section")?;
        if matches.next().is_some() {
            return Err("ambiguous section mapping".into());
        }
        Ok(s)
    }
    pub fn offset(&self, rva: u32, len: usize) -> Result<usize, String> {
        let s = self.section(rva, len)?;
        s.raw
            .checked_add((rva - s.rva) as usize)
            .ok_or("file offset overflow".into())
    }
    pub fn virtual_data_address(&self, rva: u32, len: u32) -> Result<(), String> {
        let end = rva.checked_add(len).ok_or("virtual data RVA overflow")?;
        let mut matches = self
            .sections
            .iter()
            .filter(|s| rva >= s.rva && end <= s.rva + s.virtual_len);
        let section = matches
            .next()
            .ok_or("virtual data address outside image sections")?;
        if matches.next().is_some()
            || section.flags & 0x4000_0000 == 0
            || section.flags & 0x2000_0000 != 0
        {
            return Err("virtual data address ambiguous or not readable data".into());
        }
        Ok(())
    }
    fn span_any(&self, rva: u32, len: usize) -> Result<&'a [u8], String> {
        at(self.bytes, self.offset(rva, len)?, len)
    }
    pub fn span(&self, rva: u32, len: usize, code: bool) -> Result<&'a [u8], String> {
        let s = self.section(rva, len)?;
        if s.flags & 0x4000_0000 == 0 || (s.flags & 0x2000_0000 != 0) != code {
            return Err("operation code/data section mismatch".into());
        }
        self.span_any(rva, len)
    }
    pub fn chain_root(&self, mut f: Function) -> Result<Function, String> {
        let mut seen = Vec::new();
        for _ in 0..16 {
            if seen.contains(&f) {
                return Err("operation CHAININFO cycle".into());
            }
            seen.push(f);
            let h = self.span(f.unwind, 4, false)?;
            if h[0] & 7 != 1 || h[0] >> 3 & !7 != 0 {
                return Err("operation unwind version/flags".into());
            }
            let aligned = 4 + ((usize::from(h[2]) + 1) & !1) * 2;
            self.span(f.unwind, aligned, false)?;
            if h[0] >> 3 & 4 == 0 {
                return Ok(f);
            }
            if h[0] >> 3 & 3 != 0 {
                return Err("chained unwind also has exception handler".into());
            }
            let p = f
                .unwind
                .checked_add(aligned as u32)
                .ok_or("unwind overflow")?;
            let row = self.span(p, 12, false)?;
            f = Function {
                begin: u32_at(row, 0)?,
                end: u32_at(row, 4)?,
                unwind: u32_at(row, 8)?,
            };
            if !self.functions.contains(&f) {
                return Err("operation CHAININFO parent missing from pdata".into());
            }
        }
        Err("operation CHAININFO depth exceeded".into())
    }
    pub fn pointer(&self, slot: u32, target: u32, relocated: bool) -> Result<(), String> {
        if u64_at(self.span(slot, 8, false)?, 0)?.checked_sub(self.base) != Some(u64::from(target))
            || (relocated && !self.relocations.contains(&slot))
        {
            return Err(format!("operation VFT/DIR64 differs at {slot:#x}"));
        }
        self.span(target, 1, true)?;
        Ok(())
    }
}
