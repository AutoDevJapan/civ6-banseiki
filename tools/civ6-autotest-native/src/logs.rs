//! Judge only bytes written since a captured log generation and byte offset.
//!
//! Missing files at capture time are allowed (a game may not have created them
//! yet). At judging time all three files are mandatory. Rotation, truncation,
//! or a changed saved prefix/tail causes the entire new generation to be read.

use chrono::Utc;
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub const LOG_NAMES: [&str; 3] = ["Lua.log", "Database.log", "Modding.log"];
const HASH_WINDOW: u64 = 256;

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    inode: u64,
    device: u64,
}

// The supplied windows-sys features do not include Storage_FileSystem. Keep
// this small read-only ABI here instead of changing the shared Cargo manifest.
// Its layout/signature follows windows-sys 0.61.2's primary implementation.
#[cfg(windows)]
mod windows_identity {
    use super::FileIdentity;
    use std::fs::File;
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{FILETIME, HANDLE};

    #[repr(C)]
    #[derive(Default)]
    struct ByHandleFileInformation {
        attributes: u32,
        creation: FILETIME,
        last_access: FILETIME,
        last_write: FILETIME,
        volume_serial: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(
            handle: HANDLE,
            information: *mut ByHandleFileInformation,
        ) -> i32;
    }

    pub(super) fn identity(file: &File) -> io::Result<FileIdentity> {
        let mut information = ByHandleFileInformation::default();
        // SAFETY: the File owns a live handle for this call, and information is
        // an initialized, correctly laid-out writable Win32 output structure.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(FileIdentity {
            inode: (u64::from(information.index_high) << 32) | u64::from(information.index_low),
            device: u64::from(information.volume_serial),
        })
    }
}

#[cfg(windows)]
fn file_identity(file: &File) -> io::Result<FileIdentity> {
    windows_identity::identity(file)
}

#[cfg(unix)]
fn file_identity(file: &File) -> io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    Ok(FileIdentity {
        inode: metadata.ino(),
        device: metadata.dev(),
    })
}

#[cfg(not(any(unix, windows)))]
fn file_identity(_file: &File) -> io::Result<FileIdentity> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "cannot identify log file generations on this OS",
    ))
}

fn sha256(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").expect("writing a String cannot fail");
    }
    hex
}

fn file_error(path: &Path, operation: &str, error: impl std::fmt::Display) -> String {
    format!("Cannot {operation} Civ6 log {}: {error}", path.display())
}

/// Capture a generation identifier, byte offset, and hashes of both saved ends.
/// The fields retain the existing judge.py baseline names; `device` additionally
/// disambiguates identical file numbers on different volumes.
pub fn snapshot(dir: &Path) -> Result<Value, String> {
    let mut files = Map::new();
    for name in LOG_NAMES {
        let path = dir.join(name);
        let mut file = match File::open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                files.insert(
                    name.into(),
                    json!({
                        "offset": 0, "inode": null, "device": null,
                        "prefix_length": 0, "prefix_sha256": null,
                        "tail_length": 0, "tail_sha256": null,
                    }),
                );
                continue;
            }
            Err(e) => return Err(file_error(&path, "open", e)),
        };
        let metadata = file
            .metadata()
            .map_err(|e| file_error(&path, "inspect", e))?;
        if !metadata.is_file() {
            return Err(format!(
                "Required Civ6 log is not a file: {}",
                path.display()
            ));
        }
        let identity = file_identity(&file).map_err(|e| file_error(&path, "identify", e))?;
        let offset = metadata.len();
        let length = HASH_WINDOW.min(offset);
        let mut prefix = vec![0; length as usize];
        let mut tail = vec![0; length as usize];
        file.read_exact(&mut prefix)
            .map_err(|e| file_error(&path, "capture prefix of", e))?;
        file.seek(SeekFrom::Start(offset - length))
            .map_err(|e| file_error(&path, "seek", e))?;
        file.read_exact(&mut tail)
            .map_err(|e| file_error(&path, "capture tail of", e))?;
        if file
            .metadata()
            .map_err(|e| file_error(&path, "inspect", e))?
            .len()
            < offset
        {
            return Err(format!(
                "Log was truncated while capturing the baseline: {}",
                path.display()
            ));
        }
        verify_generation(&path, identity)?;
        files.insert(
            name.into(),
            json!({
                "offset": offset, "inode": identity.inode, "device": identity.device,
                "prefix_length": length, "prefix_sha256": sha256(&prefix),
                "tail_length": length, "tail_sha256": sha256(&tail),
            }),
        );
    }
    Ok(json!({
        "created_at": Utc::now().to_rfc3339(),
        "log_dir": dir.to_string_lossy(),
        "files": files,
    }))
}

fn verify_generation(path: &Path, expected: FileIdentity) -> Result<(), String> {
    let current = File::open(path).map_err(|e| file_error(path, "reopen", e))?;
    let identity = file_identity(&current).map_err(|e| file_error(path, "identify", e))?;
    if identity != expected {
        return Err(format!(
            "Log generation changed during the read: {}",
            path.display()
        ));
    }
    Ok(())
}

struct Marker {
    offset: u64,
    inode: Option<u64>,
    device: Option<u64>,
    prefix_length: usize,
    prefix_sha256: Option<String>,
    tail_length: usize,
    tail_sha256: Option<String>,
}

impl Marker {
    fn parse(name: &str, value: &Value) -> Result<Self, String> {
        let bad = || format!("Invalid or incomplete baseline marker for {name}");
        let object = value.as_object().ok_or_else(bad)?;
        let uint = |key: &str| object.get(key).and_then(Value::as_u64).ok_or_else(bad);
        let optional_uint = |key: &str| -> Result<Option<u64>, String> {
            let value = object.get(key).ok_or_else(bad)?;
            if value.is_null() {
                Ok(None)
            } else {
                value.as_u64().map(Some).ok_or_else(bad)
            }
        };
        let hash = |key: &str| -> Result<Option<String>, String> {
            let value = object.get(key).ok_or_else(bad)?;
            if value.is_null() {
                return Ok(None);
            }
            let text = value.as_str().ok_or_else(bad)?;
            if text.len() != 64 || !text.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(bad());
            }
            Ok(Some(text.to_ascii_lowercase()))
        };
        let offset = uint("offset")?;
        let inode = optional_uint("inode")?;
        // Older Python snapshots had no device; their inode and both hashes
        // still allow the same generation checks without inventing an identity.
        let device = if object.contains_key("device") {
            optional_uint("device")?
        } else {
            None
        };
        let prefix_length = uint("prefix_length")?;
        let tail_length = uint("tail_length")?;
        let prefix_sha256 = hash("prefix_sha256")?;
        let tail_sha256 = hash("tail_sha256")?;
        if inode.is_none() {
            if offset != 0
                || device.is_some()
                || prefix_length != 0
                || tail_length != 0
                || prefix_sha256.is_some()
                || tail_sha256.is_some()
            {
                return Err(bad());
            }
        } else if prefix_length != HASH_WINDOW.min(offset)
            || tail_length != HASH_WINDOW.min(offset)
            || prefix_sha256.is_none()
            || tail_sha256.is_none()
        {
            return Err(bad());
        }
        Ok(Self {
            offset,
            inode,
            device,
            prefix_length: prefix_length as usize,
            prefix_sha256,
            tail_length: tail_length as usize,
            tail_sha256,
        })
    }

    fn start(&self, contents: &[u8], identity: FileIdentity) -> usize {
        if self.inode.is_none()
            || self.offset > contents.len() as u64
            || self.inode != Some(identity.inode)
            || self.device.is_some_and(|device| device != identity.device)
        {
            return 0;
        }
        // offset now fits inside this byte buffer; parse validated both lengths.
        let offset = self.offset as usize;
        if self.prefix_sha256.as_deref() != Some(sha256(&contents[..self.prefix_length]).as_str())
            || self.tail_sha256.as_deref()
                != Some(sha256(&contents[offset - self.tail_length..offset]).as_str())
        {
            return 0;
        }
        offset
    }
}

/// Read all three required logs from the saved offset, or from byte zero when
/// the captured generation no longer matches. Invalid baselines are errors.
pub fn read_since(dir: &Path, baseline: &Value) -> Result<BTreeMap<String, String>, String> {
    let files = baseline
        .get("files")
        .and_then(Value::as_object)
        .ok_or_else(|| "Baseline must contain an object named files".to_string())?;
    let markers: Vec<_> = LOG_NAMES
        .iter()
        .map(|name| {
            let value = files
                .get(*name)
                .ok_or_else(|| format!("Missing baseline marker for {name}"))?;
            Marker::parse(name, value)
        })
        .collect::<Result<_, String>>()?;
    let mut result = BTreeMap::new();
    for (name, marker) in LOG_NAMES.into_iter().zip(markers) {
        let path = dir.join(name);
        let mut file = File::open(&path).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                format!("Required Civ6 log is missing: {}", path.display())
            } else {
                file_error(&path, "open", e)
            }
        })?;
        let metadata = file
            .metadata()
            .map_err(|e| file_error(&path, "inspect", e))?;
        if !metadata.is_file() {
            return Err(format!(
                "Required Civ6 log is not a file: {}",
                path.display()
            ));
        }
        let identity = file_identity(&file).map_err(|e| file_error(&path, "identify", e))?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .map_err(|e| file_error(&path, "read", e))?;
        if file
            .metadata()
            .map_err(|e| file_error(&path, "inspect", e))?
            .len()
            < contents.len() as u64
        {
            return Err(format!(
                "Log was truncated during the read: {}",
                path.display()
            ));
        }
        verify_generation(&path, identity)?;
        let bytes = &contents[marker.start(&contents, identity)..];
        let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
        result.insert(
            name.to_string(),
            String::from_utf8_lossy(bytes).into_owned(),
        );
    }
    Ok(result)
}

fn regex(pattern: &str) -> Result<Regex, String> {
    Regex::new(pattern).map_err(|e| format!("Cannot construct log judge pattern: {e}"))
}

fn ignored_error_value(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["none", "nil"].iter().any(|word| {
        lower.strip_prefix(*word).is_some_and(|suffix| {
            suffix
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_')
        })
    })
}

fn jac_error_line(line: &str, failure: &Regex, error_word: &Regex) -> bool {
    let Some((_, body)) = line.split_once("[JAC_POC]") else {
        return false;
    };
    if failure.is_match(body) {
        return true;
    }
    for word in error_word.find_iter(body) {
        let suffix = body[word.end()..].trim_start();
        if let Some(value) = suffix.strip_prefix('=')
            && ignored_error_value(value.trim_start())
        {
            continue;
        }
        return true;
    }
    false
}

fn runtime_errors<'a>(lua: &'a str, runtime: &Regex, jac: &Regex) -> Vec<&'a str> {
    let lines: Vec<_> = lua.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !runtime.is_match(line) {
            continue;
        }
        let mut context = (*line).to_string();
        // Keep the entire contiguous stack, including JAC frames beyond the
        // old eight-line limit, but stop before the next unrelated log message.
        for stack in &lines[index + 1..] {
            if stack.chars().next().is_some_and(char::is_whitespace)
                || stack
                    .trim_start()
                    .to_lowercase()
                    .starts_with("stack traceback")
            {
                context.push('\n');
                context.push_str(stack);
            } else {
                break;
            }
        }
        if jac.is_match(&context) {
            found.push(*line);
        }
    }
    found
}

/// Existing generic error/texture checks plus mandatory literal log fragments.
/// Each `require_logs` entry must occur in at least one of the three new log
/// slices. Empty entries are invalid, and an empty Lua slice cannot prove a run.
pub fn judge_generic(
    logs: &BTreeMap<String, String>,
    require_logs: &[String],
) -> Result<Vec<Check>, String> {
    for name in LOG_NAMES {
        if !logs.contains_key(name) {
            return Err(format!("Missing log: {name}"));
        }
    }
    if require_logs.iter().any(|text| text.trim().is_empty()) {
        return Err("Required log fragments must not be empty".into());
    }
    let lua = &logs["Lua.log"];
    if lua.trim().is_empty() {
        return Err("No new Lua log evidence; cannot judge an empty run".into());
    }
    let failure = regex(r"(?i)\b(?:api_error|failed|failure|exception|runtime\s+error)\b")?;
    let error_word = regex(r"(?i)\berror\b")?;
    let runtime = regex(r"(?i)runtime error")?;
    let jac = regex(r"(?i)\bJAC[_\\/.-]|JAC_PoC")?;
    let errors: Vec<_> = lua
        .lines()
        .filter(|line| jac_error_line(line, &failure, &error_word))
        .collect();
    let traces = runtime_errors(lua, &runtime, &jac);
    let missing: Vec<_> = lua
        .lines()
        .filter(|line| {
            let lower = line.to_lowercase();
            lower.contains("jac")
                && [
                    "could not find icon",
                    "we are lacking a texture",
                    "failed to load background image texture",
                ]
                .iter()
                .any(|marker| lower.contains(*marker))
        })
        .collect();
    let mut checks = vec![
        Check {
            name: "Generic: [JAC_POC] errors".into(),
            passed: errors.is_empty(),
            detail: format!("{} matching lines", errors.len()),
        },
        Check {
            name: "Generic: JAC Runtime Error".into(),
            passed: traces.is_empty(),
            detail: format!("{} matching stack traces", traces.len()),
        },
        Check {
            name: "Generic: JAC missing texture".into(),
            passed: missing.is_empty(),
            detail: format!("{} matching lines", missing.len()),
        },
    ];
    for required in require_logs {
        let matched: Vec<_> = LOG_NAMES
            .iter()
            .filter(|name| logs[**name].contains(required.as_str()))
            .copied()
            .collect();
        checks.push(Check {
            name: format!("Scenario: required log {required}"),
            passed: !matched.is_empty(),
            detail: if matched.is_empty() {
                "Required fragment was not found in any new log bytes".into()
            } else {
                format!("Found in {}", matched.join(", "))
            },
        });
    }
    Ok(checks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct FixtureDir(PathBuf);
    impl FixtureDir {
        fn new() -> Self {
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "civ6-native-logs-{}-{time}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn populate(&self) {
            for name in LOG_NAMES {
                fs::write(self.0.join(name), b"").unwrap();
            }
        }
    }
    impl Drop for FixtureDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample(lua: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("Lua.log".into(), lua.into()),
            ("Database.log".into(), String::new()),
            ("Modding.log".into(), String::new()),
        ])
    }

    fn failed(checks: &[Check]) -> Vec<&str> {
        checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| c.name.as_str())
            .collect()
    }

    #[test]
    fn sha256_matches_independent_known_digest() {
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn append_uses_byte_offset_and_keeps_utf8() {
        let fixture = FixtureDir::new();
        fixture.populate();
        let path = fixture.0.join("Lua.log");
        fs::write(&path, "古い error\n").unwrap();
        let baseline = snapshot(&fixture.0).unwrap();
        assert_eq!(
            baseline["files"]["Lua.log"]["offset"],
            json!("古い error\n".len())
        );
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all("新しい行\n".as_bytes())
            .unwrap();
        let logs = read_since(&fixture.0, &baseline).unwrap();
        assert_eq!(logs["Lua.log"], "新しい行\n");
        assert_eq!(logs.len(), 3);
    }

    #[test]
    fn truncate_and_regrow_with_same_prefix_reads_whole_generation() {
        let fixture = FixtureDir::new();
        fixture.populate();
        let path = fixture.0.join("Lua.log");
        let original = [vec![b'P'; 256], vec![b'M'; 256], vec![b'T'; 256]].concat();
        fs::write(&path, &original).unwrap();
        let baseline = snapshot(&fixture.0).unwrap();
        fs::write(&path, b"fresh\n").unwrap();
        assert_eq!(
            read_since(&fixture.0, &baseline).unwrap()["Lua.log"],
            "fresh\n"
        );
        let replacement = [
            vec![b'P'; 256],
            vec![b'R'; 256],
            vec![b'U'; 256],
            b"fresh\n".to_vec(),
        ]
        .concat();
        fs::write(&path, &replacement).unwrap();
        assert_eq!(
            read_since(&fixture.0, &baseline).unwrap()["Lua.log"],
            String::from_utf8(replacement).unwrap()
        );
    }

    #[test]
    fn changed_prefix_with_unchanged_old_tail_reads_whole_file() {
        let fixture = FixtureDir::new();
        fixture.populate();
        let path = fixture.0.join("Lua.log");
        let original = [vec![b'P'; 256], vec![b'T'; 256]].concat();
        fs::write(&path, original).unwrap();
        let baseline = snapshot(&fixture.0).unwrap();
        let replacement = [vec![b'Q'; 256], vec![b'T'; 256], b"new\n".to_vec()].concat();
        fs::write(&path, &replacement).unwrap();
        assert_eq!(
            read_since(&fixture.0, &baseline).unwrap()["Lua.log"],
            String::from_utf8(replacement).unwrap()
        );
    }

    #[test]
    fn replacing_file_with_identical_hashed_ends_still_reads_all_bytes() {
        let fixture = FixtureDir::new();
        fixture.populate();
        let path = fixture.0.join("Lua.log");
        let original = [vec![b'P'; 256], vec![b'M'; 256], vec![b'T'; 256]].concat();
        fs::write(&path, original).unwrap();
        let baseline = snapshot(&fixture.0).unwrap();
        let replacement = [
            vec![b'P'; 256],
            vec![b'R'; 256],
            vec![b'T'; 256],
            b"new\n".to_vec(),
        ]
        .concat();
        let stage = fixture.0.join("replacement.log");
        fs::write(&stage, &replacement).unwrap();
        fs::remove_file(&path).unwrap();
        fs::rename(&stage, &path).unwrap();
        assert_eq!(
            read_since(&fixture.0, &baseline).unwrap()["Lua.log"],
            String::from_utf8(replacement).unwrap()
        );
    }

    #[test]
    fn absent_snapshot_logs_become_entire_new_files() {
        let fixture = FixtureDir::new();
        let baseline = snapshot(&fixture.0).unwrap();
        assert_eq!(baseline["files"]["Lua.log"]["inode"], Value::Null);
        for name in LOG_NAMES {
            fs::write(fixture.0.join(name), b"new log\n").unwrap();
        }
        let logs = read_since(&fixture.0, &baseline).unwrap();
        assert_eq!(
            logs,
            BTreeMap::from([
                ("Lua.log".into(), "new log\n".into()),
                ("Database.log".into(), "new log\n".into()),
                ("Modding.log".into(), "new log\n".into()),
            ])
        );
    }

    #[test]
    fn missing_current_file_or_incomplete_baseline_cannot_pass() {
        let fixture = FixtureDir::new();
        fixture.populate();
        let baseline = snapshot(&fixture.0).unwrap();
        fs::remove_file(fixture.0.join("Database.log")).unwrap();
        assert!(
            read_since(&fixture.0, &baseline)
                .unwrap_err()
                .contains("Database.log")
        );
        fixture.populate();
        for key in [
            "offset",
            "inode",
            "prefix_length",
            "tail_length",
            "prefix_sha256",
            "tail_sha256",
        ] {
            let mut corrupt = baseline.clone();
            corrupt["files"]["Lua.log"]
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(read_since(&fixture.0, &corrupt).is_err(), "{key}");
        }
        let mut corrupt = baseline.clone();
        corrupt["files"]["Lua.log"]["tail_length"] = json!(257);
        assert!(read_since(&fixture.0, &corrupt).is_err());
    }

    #[test]
    fn bom_and_invalid_utf8_match_legacy_lossy_decoding() {
        let fixture = FixtureDir::new();
        fixture.populate();
        let baseline = snapshot(&fixture.0).unwrap();
        fs::write(fixture.0.join("Lua.log"), b"\xef\xbb\xbfnew\xff\n").unwrap();
        assert_eq!(
            read_since(&fixture.0, &baseline).unwrap()["Lua.log"],
            "new\u{fffd}\n"
        );
    }

    #[test]
    fn error_none_and_nil_are_allowed_but_real_error_tokens_fail() {
        let logs = sample("JAC: [JAC_POC] result=unavailable error=none\n[JAC_POC] error = NIL\n");
        let checks = judge_generic(&logs, &[]).unwrap();
        assert_eq!(checks.len(), 3);
        assert!(failed(&checks).is_empty());
        let failure =
            regex(r"(?i)\b(?:api_error|failed|failure|exception|runtime\s+error)\b").unwrap();
        let error = regex(r"(?i)\berror\b").unwrap();
        for text in [
            "ERROR: boom",
            "api_error",
            "failed",
            "failure",
            "exception",
            "runtime error",
            "error=none error=boom",
            "error=none_suffix",
            "error=nilpotent",
            "error=",
        ] {
            assert!(
                jac_error_line(&format!("[JAC_POC] {text}"), &failure, &error),
                "{text}"
            );
        }
        assert!(!jac_error_line(
            "Runtime Error: official panel\n",
            &failure,
            &error
        ));
    }

    #[test]
    fn jac_runtime_stack_is_attributed_without_consuming_next_message() {
        let logs = sample(
            "[JAC_POC] loaded\nRuntime Error: bad argument\n  [C]: pcall\n  Mods/JAC_PoC_CustomCivs/Scripts.lua:4\nRuntime Error: official panel\n  BaseGamePanel.lua:21\nJAC: [JAC_POC] restored\n",
        );
        let checks = judge_generic(&logs, &[]).unwrap();
        assert_eq!(failed(&checks), vec!["Generic: JAC Runtime Error"]);
        assert_eq!(checks[1].detail, "1 matching stack traces");
        let mut long = "Runtime Error: deep stack\nstack traceback:\n".to_string();
        for _ in 0..12 {
            long.push_str("  BaseGamePanel.lua:21\n");
        }
        long.push_str("  Mods/JAC_Sengoku/Scripts.lua:4\n");
        assert_eq!(
            failed(&judge_generic(&sample(&long), &[]).unwrap()),
            vec!["Generic: JAC Runtime Error"]
        );
    }

    #[test]
    fn jac_missing_icons_and_textures_fail_and_official_art_is_allowed() {
        let logs = sample(
            "[JAC_POC] loaded\nDiplomacyRibbon: Could not find icon ICON_CIVILIZATION_CHINA\nLoadScreen: We are lacking a texture for LEADER_HOJO_NEUTRAL\n",
        );
        assert!(failed(&judge_generic(&logs, &[]).unwrap()).is_empty());
        let logs = sample(
            "DiplomacyRibbon: Could not find icon ICON_CIVILIZATION_JAC_AINU\nLoadScreen: Failed to load background image texture: LEADER_JAC_MORI_BACKGROUND\nLoadScreen: We are lacking a texture for LEADER_JAC_MORI_NEUTRAL\n",
        );
        let checks = judge_generic(&logs, &[]).unwrap();
        assert_eq!(failed(&checks), vec!["Generic: JAC missing texture"]);
        assert_eq!(checks[2].detail, "3 matching lines");
    }

    #[test]
    fn every_required_literal_is_mandatory_across_three_logs() {
        let mut logs = sample("[JAC_POC] restored count=2\n");
        logs.insert(
            "Modding.log".into(),
            "Applying Component - JAC_Sengoku\n".into(),
        );
        logs.insert("Database.log".into(), "Passed Validation.\n".into());
        let required = vec![
            "restored count=2".into(),
            "Applying Component".into(),
            "Passed Validation.".into(),
        ];
        let checks = judge_generic(&logs, &required).unwrap();
        assert_eq!(checks.len(), 6);
        assert!(failed(&checks).is_empty());
        let mut required = required;
        required.push("absorption complete".into());
        let checks = judge_generic(&logs, &required).unwrap();
        assert_eq!(
            failed(&checks),
            vec!["Scenario: required log absorption complete"]
        );
        assert!(judge_generic(&logs, &[String::new()]).is_err());
        assert!(judge_generic(&logs, &["  ".into()]).is_err());
    }

    #[test]
    fn missing_logs_and_zero_evidence_are_errors_not_success() {
        assert!(judge_generic(&BTreeMap::new(), &[]).is_err());
        assert!(judge_generic(&sample(""), &[]).is_err());
        assert!(judge_generic(&sample("  \n"), &[]).is_err());
        let mut logs = sample("[JAC_POC] loaded\n");
        logs.remove("Modding.log");
        assert!(
            judge_generic(&logs, &[])
                .unwrap_err()
                .contains("Modding.log")
        );
    }

    #[test]
    fn checks_serialize_using_root_result_fields() {
        let check = Check {
            name: "test".into(),
            passed: false,
            detail: "missing".into(),
        };
        assert_eq!(
            serde_json::to_value(check).unwrap(),
            json!({"name": "test", "passed": false, "detail": "missing"})
        );
    }
}
