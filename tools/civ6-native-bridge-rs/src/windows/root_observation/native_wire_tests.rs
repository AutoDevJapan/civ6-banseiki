use super::*;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::process::CommandExt;

#[test]
#[ignore = "explicit licensed local primary-image acceptance"]
fn snapshot_whole_code_and_unwind_primary_bytes_reject_mutation() {
    let mut file = pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA).unwrap();
    fn offset(file: &mut std::fs::File, rva: u32) -> u64 {
        let mut word = [0; 4];
        file.seek(SeekFrom::Start(0x3c)).unwrap();
        file.read_exact(&mut word).unwrap();
        let nt = u32::from_le_bytes(word) as u64;
        let mut short = [0; 2];
        file.seek(SeekFrom::Start(nt + 6)).unwrap();
        file.read_exact(&mut short).unwrap();
        let n = u16::from_le_bytes(short);
        file.seek(SeekFrom::Start(nt + 20)).unwrap();
        file.read_exact(&mut short).unwrap();
        let opt = u16::from_le_bytes(short) as u64;
        for index in 0..n {
            file.seek(SeekFrom::Start(nt + 24 + opt + u64::from(index) * 40))
                .unwrap();
            let mut row = [0; 40];
            file.read_exact(&mut row).unwrap();
            let start = u32::from_le_bytes(row[12..16].try_into().unwrap());
            let len = u32::from_le_bytes(row[16..20].try_into().unwrap());
            if rva >= start && rva - start < len {
                return u64::from(u32::from_le_bytes(row[20..24].try_into().unwrap()))
                    + u64::from(rva - start);
            }
        }
        panic!("missing primary RVA");
    }
    for frame in fixed::FRAMES {
        let location = offset(&mut file, frame.begin);
        file.seek(SeekFrom::Start(location)).unwrap();
        let mut code = vec![0; (frame.end - frame.begin) as usize];
        file.read_exact(&mut code).unwrap();
        let expected: Vec<_> = (0..frame.specimen.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&frame.specimen[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(code, expected);
        assert_eq!(format!("{:x}", Sha256::digest(&code)), frame.sha);
        let end = code.len() - 1;
        code[end] ^= 1;
        assert_ne!(format!("{:x}", Sha256::digest(&code)), frame.sha);
        for part in frame.parts {
            let location = offset(&mut file, part.unwind);
            file.seek(SeekFrom::Start(location)).unwrap();
            let expected: Vec<_> = (0..part.hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&part.hex[i..i + 2], 16).unwrap())
                .collect();
            let mut data = vec![0; expected.len()];
            file.read_exact(&mut data).unwrap();
            assert_eq!(data, expected);
        }
    }
}
fn child(name: &str) -> bool {
    const ENV: &str = "CIV6_NATIVE_WIRE_CHILD";
    if std::env::var_os(ENV).is_some() {
        return true;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .creation_flags(0x08000000)
        .env(ENV, "1")
        .args([
            "--exact",
            &format!("windows::root_observation::native_frames::wire::tests::{name}"),
            "--nocapture",
        ])
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            return false;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("wire child timeout");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
struct Fixture {
    module: usize,
    message: [u64; 12],
    session: [u64; 4],
    owner: [u64; 2],
}
impl Fixture {
    fn new() -> Box<Self> {
        let module = unsafe {
            VirtualAlloc(
                null_mut(),
                0x1050000,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        } as usize;
        assert_ne!(module, 0);
        EXE.store(module as u64, Ordering::Release);
        let mut f = Box::new(Self {
            module,
            message: [0; 12],
            session: [0; 4],
            owner: [0; 2],
        });
        f.message[0] = (module + 0x10417e8) as u64;
        f.message[3] = (5u64 << 32) | 0x4e8;
        f.message[4] = 7;
        f.message[5] = (11u64 << 32) | 9;
        f.message[7] = 13;
        f.message[8] = 0x1000;
        f.message[9] = 0x1100;
        f.message[11] = 0x100;
        f.session[3] = f.owner.as_ptr() as u64;
        f.owner[0] = (module + 0xfd35c8) as u64;
        f.owner[1] = f.session.as_ptr() as u64;
        f
    }
    fn raw(&self) -> Saved {
        let mut raw: Saved = unsafe { std::mem::zeroed() };
        raw.gpr[0] = self.message.as_ptr() as u64;
        raw.gpr[1] = self.session.as_ptr() as u64;
        raw.invocation.caller_rsp = 0x2000;
        raw
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_ne!(unsafe { VirtualFree(self.module as _, 0, MEM_RELEASE) }, 0);
    }
}
#[test]
fn snapshot_header_is_copied_without_minting_world_and_reuse_gets_another_serial() {
    if !child("snapshot_header_is_copied_without_minting_world_and_reuse_gets_another_serial") {
        return;
    }
    let mut f = Fixture::new();
    let mut raw = f.raw();
    begin(&mut raw);
    let mut request: Saved = unsafe { std::mem::zeroed() };
    request.invocation.caller = (f.module + 0x80f297) as u64;
    request.gpr = [f.owner.as_ptr() as u64, 0x1000, 0x100, 13, 0, 0, 0, 0];
    let old = super::request(&request).unwrap();
    assert_eq!(old.session, None);
    assert_eq!(old.header[4..6], [9, 11]);
    f.message[5] = 99;
    assert_eq!(super::request(&request).unwrap(), old);
    finish(&mut raw, false);
    assert!(super::request(&request).is_none());
    let mut next = f.raw();
    begin(&mut next);
    let new = super::request(&request).unwrap();
    assert_ne!(old.serial, new.serial);
    assert_ne!(old.header, new.header);
    finish(&mut next, false);
    assert!(!FAILED.load(Ordering::Acquire));
}
#[test]
fn unknown_snapshot_is_transparent_and_known_terminal_unwind_is_unresolved() {
    if !child("unknown_snapshot_is_transparent_and_known_terminal_unwind_is_unresolved") {
        return;
    }
    let f = Fixture::new();
    let mut unknown: Saved = unsafe { std::mem::zeroed() };
    begin(&mut unknown);
    finish(&mut unknown, true);
    assert!(!FAILED.load(Ordering::Acquire));
    let mut known = f.raw();
    begin(&mut known);
    finish(&mut known, true);
    assert!(FAILED.load(Ordering::Acquire));
    assert!(CALLS.with(|s| s.borrow().is_empty()));
}
#[test]
fn unknown_snapshot_cannot_borrow_a_live_sessions_current_world() {
    if !child("unknown_snapshot_cannot_borrow_a_live_sessions_current_world") {
        return;
    }
    let f = Fixture::new();
    let lease =
        sessions::fixture_snapshot_session(f.owner.as_ptr() as u64, f.session.as_ptr() as u64);
    assert_eq!(
        sessions::snapshot_owner(f.owner.as_ptr() as u64, f.session.as_ptr() as u64)
            .unwrap()
            .0,
        lease
    );
    let snapshot = capture(f.message.as_ptr() as u64, f.session.as_ptr() as u64, 1).unwrap();
    assert_eq!(snapshot.session, None);
    assert_eq!(snapshot.packet, None);
    assert!(snapshot.matches_session(None));
}
global_asm!(
    ".globl wire_owned_original",
    ".seh_proc wire_owned_original",
    "wire_owned_original:",
    "mov [rsp+0x10],rsi",
    ".seh_savereg rsi,0x10",
    ".seh_endprologue",
    "inc qword ptr [rcx]",
    "mov eax,0x445566",
    "ret",
    ".seh_endproc"
);
unsafe extern "system" {
    fn wire_owned_original();
}
#[test]
fn published_snapshot_entry_executes_original_once_and_restores() {
    if !child("published_snapshot_entry_executes_original_once_and_restores") {
        return;
    }
    let page = unsafe {
        VirtualAlloc(
            null_mut(),
            CODE_PAGE,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as usize;
    assert_ne!(page, 0);
    // A small owned original uses the exact stolen instruction and resumes
    // after it. The suffix is never executed by this fixture.
    unsafe {
        ptr::copy_nonoverlapping(ORIGINAL.as_ptr(), page as *mut u8, 8);
    }
    let mut old = 0;
    assert_ne!(
        unsafe { VirtualProtect(page as _, CODE_PAGE, PAGE_EXECUTE_READ, &mut old) },
        0
    );
    unsafe { install_at(page) }.unwrap();
    NATIVE_WIRE_CONTINUE.store(
        wire_owned_original as *const () as u64 + 5,
        Ordering::Release,
    );
    let invoke: unsafe extern "system" fn(*mut u64, u64) -> u64 =
        unsafe { std::mem::transmute(page) };
    let mut calls = 0;
    assert_eq!(unsafe { invoke(&mut calls, 0) }, 0x445566);
    assert_eq!(calls, 1);
    unsafe { restore() }.unwrap();
    assert_eq!(copy_current::<8>(page as u64), Some(ORIGINAL));
    assert_ne!(unsafe { VirtualFree(page as _, 0, MEM_RELEASE) }, 0);
}
