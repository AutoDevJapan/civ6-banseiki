//! Explicit licensed-image preparation without executing native game bodies.
use super::*;
use windows_sys::Win32::System::Diagnostics::Debug::{RtlAddFunctionTable, RtlDeleteFunctionTable};

struct Image {
    base: usize,
    pdata: *mut IMAGE_RUNTIME_FUNCTION_ENTRY,
}
impl Image {
    fn new() -> Self {
        let mut f = super::super::super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
        let mut bytes = Vec::new();
        f.seek(SeekFrom::Start(0)).unwrap();
        f.read_to_end(&mut bytes).unwrap();
        let u16_at = |p: usize| u16::from_le_bytes(bytes[p..p + 2].try_into().unwrap());
        let u32_at = |p: usize| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap());
        let nt = u32_at(0x3c) as usize;
        let opt = nt + 24;
        let preferred = u64::from_le_bytes(bytes[opt + 24..opt + 32].try_into().unwrap()) as usize;
        let len = u32_at(opt + 56) as usize;
        // This owned child must have the fixed image's preferred range free.
        // No pointer or code mutation substitutes for the primary image.
        let base = unsafe {
            VirtualAlloc(
                preferred as _,
                len,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        } as usize;
        assert_eq!(
            base, preferred,
            "owned primary preferred address unavailable"
        );
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), base as *mut u8, u32_at(opt + 60) as usize);
        }
        let mut text = (0, 0);
        for i in 0..usize::from(u16_at(nt + 6)) {
            let row = opt + usize::from(u16_at(nt + 20)) + i * 40;
            let start = u32_at(row + 12) as usize;
            let raw = u32_at(row + 20) as usize;
            let n = u32_at(row + 16) as usize;
            assert!(start + n <= len && raw + n <= bytes.len());
            unsafe {
                ptr::copy_nonoverlapping(bytes[raw..].as_ptr(), (base + start) as *mut u8, n);
            }
            if &bytes[row..row + 5] == b".text" {
                text = (start, n);
            }
        }
        assert!(text.1 > 0);
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect((base + text.0) as _, text.1, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        let pdata =
            (base + u32_at(opt + 112 + 3 * 8) as usize) as *mut IMAGE_RUNTIME_FUNCTION_ENTRY;
        assert!(unsafe { RtlAddFunctionTable(pdata, u32_at(opt + 116 + 3 * 8) / 12, base as u64) });
        Self { base, pdata }
    }
}
impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            let _ = RtlDeleteFunctionTable(self.pdata);
            let _ = VirtualFree(self.base as _, 0, MEM_RELEASE);
        }
    }
}

#[test]
#[ignore = "explicit licensed local primary-image owned preparation"]
fn primary_early_host_prepare_after_existing_owners_publishes_scheduler_and_restores() {
    if !child("early_prepare::primary_early_host_prepare_after_existing_owners_publishes_scheduler_and_restores") {
        return;
    }
    let image = Image::new();
    assert!(FRAMES.get().is_none());
    let start = Instant::now();
    let prepare = unsafe { super::super::super::prepare_host(image.base) };
    println!(
        "OWNED_PRIMARY_HOST_PREPARE result={prepare:?} elapsed={:?}",
        start.elapsed()
    );
    let published = HOOKS.lock().unwrap().len();
    let restore = unsafe { super::super::super::restore() };
    println!("OWNED_PRIMARY_HOST_RESTORE result={restore:?} scheduler_owners={published}");
    prepare.unwrap();
    assert_eq!(published, 8);
    restore.unwrap();
    assert!(FRAMES.get().is_none());
    assert!(!FAILED.load(Ordering::Acquire));
    for (rva, prefix, _) in SITES {
        assert_eq!(copy_current::<8>((image.base + rva) as u64), Some(prefix));
    }
}
