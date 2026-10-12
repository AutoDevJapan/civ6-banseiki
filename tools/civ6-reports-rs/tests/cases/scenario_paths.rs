use super::support;
use serde_json::Value;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::Duration,
};

fn spawn_copied<T>(
    mut spawn: impl FnMut() -> io::Result<T>,
    mut pause: impl FnMut(Duration),
) -> io::Result<T> {
    for attempt in 0..10 {
        match spawn() {
            Err(error) if error.kind() == io::ErrorKind::ExecutableFileBusy && attempt < 9 => {
                // Linux can briefly refuse exec of a freshly copied file while
                // another concurrently spawned child still holds a write fd.
                // exec failed before starting the program; other errors and
                // errors from an already started child must never be replayed.
                pause(Duration::from_millis(10));
            }
            result => return result,
        }
    }
    unreachable!()
}

fn moved_output(command: &mut Command) -> Output {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    spawn_copied(|| command.spawn(), std::thread::sleep)
        .unwrap()
        .wait_with_output()
        .unwrap()
}

#[test]
fn copied_executable_spawn_retries_only_busy_before_start() {
    let mut attempts = 0;
    let mut delays = Vec::new();
    let result = spawn_copied(
        || {
            attempts += 1;
            if attempts < 4 {
                Err(io::Error::from(io::ErrorKind::ExecutableFileBusy))
            } else {
                Ok(17)
            }
        },
        |delay| delays.push(delay),
    );
    assert_eq!(result.unwrap(), 17);
    assert_eq!(attempts, 4);
    assert_eq!(delays, vec![Duration::from_millis(10); 3]);

    let mut attempts = 0;
    let result: io::Result<()> = spawn_copied(
        || {
            attempts += 1;
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        },
        |_| panic!("non-busy errors must not wait or retry"),
    );
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(attempts, 1);
}

#[test]
fn copied_executable_spawn_preserves_busy_after_bounded_attempts() {
    let mut attempts = 0;
    let mut delays = Vec::new();
    let result: io::Result<()> = spawn_copied(
        || {
            attempts += 1;
            Err(io::Error::from(io::ErrorKind::ExecutableFileBusy))
        },
        |delay| delays.push(delay),
    );
    assert_eq!(
        result.unwrap_err().kind(),
        io::ErrorKind::ExecutableFileBusy
    );
    assert_eq!(attempts, 10);
    assert_eq!(delays, vec![Duration::from_millis(10); 9]);
}

fn checkout(parent: &Path, name: &str, marker: &str) -> PathBuf {
    let root = parent.join(name);
    let folder = root.join("tools/civ6-autotest/scenarios");
    fs::create_dir_all(&folder).unwrap();
    let mut base = support::repo_json("tools/civ6-autotest/scenarios/japan-15km-200turn-full.json");
    base["steps"][1]["branch_marker"] = marker.into();
    fs::write(
        folder.join("japan-15km-200turn-full.json"),
        serde_json::to_string_pretty(&base).unwrap(),
    )
    .unwrap();
    root
}

fn moved_binary(root: &Path) -> PathBuf {
    let original = Path::new(env!("CARGO_BIN_EXE_civ6-build-arbitration-ai-scenario"));
    let folder = root.join("tools/target/debug");
    fs::create_dir_all(&folder).unwrap();
    let binary = folder.join(original.file_name().unwrap());
    // Keep the compiled executable inode immutable. Linux can reject an
    // immediate exec of a newly written copy with ETXTBSY.
    #[cfg(unix)]
    fs::hard_link(original, &binary).unwrap();
    #[cfg(not(unix))]
    {
        fs::copy(original, &binary).unwrap();
        fs::set_permissions(&binary, fs::metadata(original).unwrap().permissions()).unwrap();
    }
    binary
}

#[test]
fn generator_moved_with_checkout_uses_executable_repository() {
    let temp = support::tempdir();
    let executable_root = checkout(temp.path(), "moved-checkout", "executable checkout");
    let other_root = checkout(temp.path(), "other-checkout", "other current directory");
    let binary = moved_binary(&executable_root);
    let output_path = temp.path().join("explicit-output.json");
    let output = moved_output(
        Command::new(&binary)
            .arg("--output")
            .arg(&output_path)
            .current_dir(&other_root),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let generated: Value = serde_json::from_slice(&fs::read(&output_path).unwrap()).unwrap();
    assert_eq!(
        generated["steps"][1]["branch_marker"],
        "executable checkout"
    );
    assert_eq!(generated["steps"][1]["seed"], 620981);

    // The default must also follow the executable, without modifying the build checkout.
    let output = moved_output(Command::new(&binary).current_dir(&other_root));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let default_path =
        executable_root.join("tools/civ6-autotest/scenarios/court-arbitration-ai.json");
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(default_path).unwrap()).unwrap(),
        generated
    );
    assert!(!other_root
        .join("tools/civ6-autotest/scenarios/court-arbitration-ai.json")
        .exists());
}

#[test]
fn generator_explicit_repo_overrides_executable_checkout_and_validates_it() {
    let temp = support::tempdir();
    let executable_root = checkout(temp.path(), "executable", "executable checkout");
    let selected_root = checkout(temp.path(), "selected", "selected with repo option");
    let binary = moved_binary(&executable_root);
    let output = moved_output(
        Command::new(&binary)
            .arg("--repo")
            .arg(&selected_root)
            .current_dir(&executable_root),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output_path = selected_root.join("tools/civ6-autotest/scenarios/court-arbitration-ai.json");
    let generated: Value = serde_json::from_slice(&fs::read(&output_path).unwrap()).unwrap();
    assert_eq!(
        generated["steps"][1]["branch_marker"],
        "selected with repo option"
    );
    assert!(!executable_root
        .join("tools/civ6-autotest/scenarios/court-arbitration-ai.json")
        .exists());
    for bad_repo in [temp.path().join("missing"), temp.path().to_path_buf()] {
        let output = moved_output(
            Command::new(&binary)
                .arg("--repo")
                .arg(&bad_repo)
                .arg("--output")
                .arg(temp.path().join("must-not-exist.json"))
                .current_dir(&executable_root),
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!temp.path().join("must-not-exist.json").exists());
    }
}
