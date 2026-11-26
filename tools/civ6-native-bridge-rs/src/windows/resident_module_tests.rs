//! Exercise the shipping pin operation inside a disposable, unloadable DLL.
use super::*;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::FreeLibrary;
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleExW, GetProcAddress, LoadLibraryW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
};

struct FixtureFiles {
    paths: Vec<PathBuf>,
}
impl Drop for FixtureFiles {
    fn drop(&mut self) {
        for path in &self.paths {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => eprintln!("owned module fixture cleanup {}: {error}", path.display()),
            }
        }
    }
}

fn build_fixture(source: &Path, dll: &Path, import: &Path) {
    let out = tempfile::NamedTempFile::new_in(source.parent().unwrap()).unwrap();
    let err = tempfile::NamedTempFile::new_in(source.parent().unwrap()).unwrap();
    let mut command = Command::new(option_env!("RUSTC").unwrap_or("rustc"));
    command
        .args([
            "--crate-type",
            "cdylib",
            "--edition=2021",
            "-C",
            "debuginfo=0",
        ])
        .arg("-C")
        .arg(format!("link-arg=/IMPLIB:{}", import.display()))
        .arg(source)
        .arg("-o")
        .arg(dll)
        .stdin(Stdio::null())
        .stdout(out.as_file().try_clone().unwrap())
        .stderr(err.as_file().try_clone().unwrap());
    // The compiler and linker are contained before the compiler can execute.
    // This reuses the build-tree owner; it does not enter a nested Cargo gate.
    let (mut child, mut tree) =
        test_process_tree::Tree::spawn(&mut command, 0x08000000, true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => {}
            Ok(None) => break Err("owned module compiler deadline".to_string()),
            Err(error) => break Err(error.to_string()),
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stopped = tree.stop(&mut child);
    let reaped = child.wait();
    drop(tree);
    drop(command);
    let mut diagnostic = String::new();
    err.reopen()
        .unwrap()
        .take(1024 * 1024)
        .read_to_string(&mut diagnostic)
        .unwrap();
    out.close().unwrap();
    err.close().unwrap();
    assert!(
        stopped.is_ok() && reaped.is_ok(),
        "owned compiler reap: {stopped:?} {reaped:?}"
    );
    assert!(
        result.as_ref().is_ok_and(|status| status.success()),
        "owned module build: {result:?}\n{diagnostic}"
    );
}

#[test]
fn pinned_module_survives_release_while_the_unpinned_control_unloads() {
    const ENV: &str = "CIV6_RESIDENT_MODULE_FIXTURE";
    if let Some(path) = std::env::var_os(ENV) {
        let path: Vec<u16> = path.encode_wide().chain(Some(0)).collect();
        let module = unsafe { LoadLibraryW(path.as_ptr()) };
        assert!(
            !module.is_null(),
            "owned DLL load: {}",
            std::io::Error::last_os_error()
        );
        let export = unsafe { GetProcAddress(module, c"pin_fixture".as_ptr().cast()) }.unwrap();
        let address = export as *const () as *const u16;
        let lookup = || {
            let mut found = null_mut();
            unsafe {
                GetModuleHandleExW(
                    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                        | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                    address,
                    &mut found,
                ) != 0
            }
        };
        assert!(lookup());
        let pinned = std::env::var_os("CIV6_RESIDENT_MODULE_PIN").is_some();
        if pinned {
            let pin: unsafe extern "system" fn() -> i32 = unsafe { std::mem::transmute(export) };
            assert_eq!(unsafe { pin() }, 1);
        }
        assert_ne!(unsafe { FreeLibrary(module) }, 0);
        assert_eq!(
            lookup(),
            pinned,
            "the explicit loader reference has been released"
        );
        println!("OWNED_MODULE_LIFETIME pinned={pinned} loader_reference_released=true");
        return;
    }
    let parent = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let stem = parent.join(format!("native_residency_fixture_{}", std::process::id()));
    let source = stem.with_extension("rs");
    let dll = stem.with_extension("dll");
    let import = stem.with_extension("lib");
    let paths = vec![
        source.clone(),
        dll.clone(),
        import.clone(),
        stem.with_extension("exp"),
        stem.with_extension("pdb"),
    ];
    assert!(
        paths.iter().all(|path| !path.exists()),
        "fixture never overwrites a preexisting file"
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&source)
        .unwrap();
    let files = FixtureFiles { paths };
    // The same compiled source is used by both shipping pin call sites and
    // this DLL; changing PIN to an uncounted lookup must fail the lifetime test.
    write!(file, "mod residency {{\n{}\n}}\n#[no_mangle]\npub extern \"system\" fn pin_fixture() -> i32 {{ residency::pin_module(pin_fixture as *const () as *const core::ffi::c_void) as i32 }}\n", include_str!("resident_module.rs")).unwrap();
    drop(file);
    build_fixture(&source, &dll, &import);
    for pinned in [false, true] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.env(ENV, &dll).env_remove("CIV6_RESIDENT_MODULE_PIN").args([
            "--exact", "windows::resident_module_tests::pinned_module_survives_release_while_the_unpinned_control_unloads", "--nocapture",
        ]);
        if pinned {
            command.env("CIV6_RESIDENT_MODULE_PIN", "1");
        }
        let output = test_child_output::run(&mut command, Duration::from_secs(20)).unwrap();
        assert!(
            output.status.success(),
            "owned DLL child: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains(&format!(
            "OWNED_MODULE_LIFETIME pinned={pinned} loader_reference_released=true"
        )));
    }
    let paths = files.paths.clone();
    drop(files);
    assert!(
        paths.iter().all(|path| !path.exists()),
        "owned compiler outputs have been removed after both children were reaped"
    );
}
