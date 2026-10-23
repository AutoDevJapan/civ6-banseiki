use super::support;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

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

fn moved_binary_tempdir() -> tempfile::TempDir {
    let original = Path::new(env!("CARGO_BIN_EXE_civ6-build-arbitration-ai-scenario"));
    // Unix fixtures hard-link the immutable executable, so they must share its filesystem.
    tempfile::tempdir_in(original.parent().unwrap()).unwrap()
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
    let temp = moved_binary_tempdir();
    let executable_root = checkout(temp.path(), "moved-checkout", "executable checkout");
    let other_root = checkout(temp.path(), "other-checkout", "other current directory");
    let binary = moved_binary(&executable_root);
    let output_path = temp.path().join("explicit-output.json");
    let output = Command::new(&binary)
        .arg("--output")
        .arg(&output_path)
        .current_dir(&other_root)
        .output()
        .unwrap();
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
    let output = Command::new(&binary)
        .current_dir(&other_root)
        .output()
        .unwrap();
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
    let temp = moved_binary_tempdir();
    let executable_root = checkout(temp.path(), "executable", "executable checkout");
    let selected_root = checkout(temp.path(), "selected", "selected with repo option");
    let binary = moved_binary(&executable_root);
    let output = Command::new(&binary)
        .arg("--repo")
        .arg(&selected_root)
        .current_dir(&executable_root)
        .output()
        .unwrap();
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
        let output = Command::new(&binary)
            .arg("--repo")
            .arg(&bad_repo)
            .arg("--output")
            .arg(temp.path().join("must-not-exist.json"))
            .current_dir(&executable_root)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!temp.path().join("must-not-exist.json").exists());
    }
}
