//! Typed NetSnapshot full body and chained pdata before observation hooks.
use super::*;
pub(super) const FRAMES:&[FixedFrame]=&[
FixedFrame{begin:0x80f230,end:0x80f2ad,sha:"bb47d1f0d6ac3dc958cee632dea51b790f33c9e5b3acfb4a0900f3145fb66b8b",#[cfg(test)] specimen:"4889742410574883ec20488bf2488bf94885d2745248895c2430488bca488b1aff93480100008bd0488bceff9358010000488b5c24304885c07404c6401b01488b4748483b4740741e448b47584585c07e15488b4e18448b4f38488b5740488b01ff90e8000000488b0dcac5b401488b7424384883c4205fe9e379feff",parts:&[
Fragment{begin:0x80f230,end:0x80f245,unwind:0x119de30,hex:"010a04000a6407000a320670"},
Fragment{begin:0x80f245,end:0x80f26b,unwind:0x11c68cc,hex:"210502000534060030f2800045f2800030de1901"},
Fragment{begin:0x80f26b,end:0x80f2ad,unwind:0x11c68e0,hex:"2100000030f2800045f2800030de1901"},
]},
];
pub(super) unsafe fn verify(module: usize) -> Result<(), &'static str> {
    for f in FRAMES {
        let len = (f.end - f.begin) as usize;
        if !committed_range(module + f.begin as usize, len, 1, false) {
            return Err("wire body range");
        }
        let bytes =
            unsafe { std::slice::from_raw_parts((module + f.begin as usize) as *const u8, len) };
        if format!("{:x}", Sha256::digest(bytes)) != f.sha {
            return Err("wire complete body differs");
        }
        for p in f.parts {
            let mut base = 0;
            let row = unsafe {
                windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry(
                    module as u64 + u64::from(p.begin),
                    &mut base,
                    null_mut(),
                )
            };
            let expected = [
                p.begin.to_le_bytes(),
                p.end.to_le_bytes(),
                p.unwind.to_le_bytes(),
            ]
            .concat();
            if base != module as u64
                || copy_current::<12>(row as u64)
                    .as_ref()
                    .map(|v| v.as_slice())
                    != Some(expected.as_slice())
            {
                return Err("wire pdata differs");
            }
            for (i, byte) in (0..p.hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&p.hex[i..i + 2], 16).unwrap())
                .enumerate()
            {
                if copy_current::<1>(module as u64 + u64::from(p.unwind) + i as u64) != Some([byte])
                {
                    return Err("wire unwind differs");
                }
            }
        }
    }
    Ok(())
}
