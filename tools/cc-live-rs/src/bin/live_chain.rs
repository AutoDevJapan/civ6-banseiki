use cc_live::{Call, Output, Paths, Runner};
use sha2::{Digest, Sha256};
#[cfg(test)]
#[path = "live_chain/scoped_tests.rs"]
mod scoped_tests;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const RESTORE_TIMEOUT: Duration = Duration::from_secs(90);
const RESTORE_INTERVAL: Duration = Duration::from_millis(500);

fn replace_option(text: &str, key: &str, value: &str) -> Result<String, String> {
    let mut count = 0;
    let mut result = String::new();
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\r', '\n']);
        if bare.split_whitespace().next() == Some(key) {
            count += 1;
            result.push_str(&format!("{key} {value}"));
            result.push_str(if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            });
        } else {
            result.push_str(line);
        }
    }
    if count != 1 {
        return Err(format!("expected one {key}, found {count}"));
    }
    Ok(result)
}
fn unique_suffix() -> String {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension(format!("warp-{}.tmp", unique_suffix()));
    // Ownership comes from successful CREATE_NEW, never from a nearby name.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| format!("create {}: {e}", tmp.display()))?;
    let written = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    let result = written.and_then(|_| fs::rename(&tmp, path));
    if let Err(error) = result {
        let cleanup = fs::remove_file(&tmp);
        return Err(format!(
            "replace {}: {error}; owned temporary cleanup={cleanup:?}",
            path.display()
        ));
    }
    Ok(())
}
fn original_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn recovery_path(lock: &Path) -> PathBuf {
    lock.with_file_name("appoptions-restore-state.json")
}
fn persist_recovery(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    // Publish durably before AppOptions can change. Existing recovery is never
    // replaced by a new run's backup, including a partial/unknown publication.
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("create recovery {}: {e}", path.display()))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("persist recovery {}: {e}", path.display()))
}
fn recover_profile(lock_path: &Path, path: &Path, expected_token: &str) -> Result<(), String> {
    let receipt = recovery_path(lock_path);
    let bytes = match fs::read(&receipt) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("read AppOptions recovery: {e}")),
    };
    let mut lock = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(lock_path)
        .map_err(|e| e.to_string())?;
    let created = self_creation()?;
    validate_owner(&read_owner(&mut lock)?, created, Some(expected_token))?;
    let state: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("invalid AppOptions recovery; launch refused: {e}"))?;
    if state["version"] != 1 || state["options"].as_str() != path.to_str() {
        return Err("AppOptions recovery target/version mismatch; launch refused".into());
    }
    let backup = PathBuf::from(state["backup"].as_str().ok_or("recovery backup missing")?);
    let hash = state["original_sha256"]
        .as_str()
        .ok_or("recovery hash missing")?;
    // Hash and consume the same bytes; never overwrite the retained backup.
    let original = fs::read(&backup).map_err(|e| format!("read recovery backup: {e}"))?;
    if original_hash(&original) != hash
        || state["original_bytes"].as_u64() != Some(original.len() as u64)
    {
        return Err("AppOptions recovery backup differs; launch refused".into());
    }
    validate_owner(&read_owner(&mut lock)?, created, Some(expected_token))?;
    atomic_write(path, &original)?;
    if fs::read(path).map_err(|e| e.to_string())? != original {
        return Err("AppOptions recovery verification failed; launch refused".into());
    }
    fs::remove_file(&receipt).map_err(|e| format!("clear AppOptions recovery: {e}"))?;
    println!(
        "PROFILE RECOVERED sha256={hash} verified=true before_new_apply=true backup={}",
        backup.display()
    );
    Ok(())
}
fn self_creation() -> Result<u64, String> {
    cc_live::chain::process_created_filetime(std::process::id())?
        .filter(|value| *value != 0)
        .ok_or_else(|| "could not verify invoking chain creation FILETIME".into())
}
fn read_owner(lock: &mut File) -> Result<serde_json::Value, String> {
    lock.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    lock.take(32769)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32768 {
        return Err("standard lease record too large".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn validate_owner(
    owner: &serde_json::Value,
    created: u64,
    token: Option<&str>,
) -> Result<String, String> {
    let pid = u64::from(std::process::id());
    if owner["live_chain"] != true
        || owner["pid"].as_u64() != Some(pid)
        || owner["chain_pid"].as_u64() != Some(pid)
        || owner["pid_created_filetime"].as_u64() != Some(created)
    {
        return Err("invoking chain PID/creation FILETIME does not own the standard lease".into());
    }
    let actual = owner["lock_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("missing standard lease token")?;
    if token.is_some_and(|expected| expected != actual) {
        return Err("standard lease changed before graphics restoration".into());
    }
    Ok(actual.to_owned())
}
struct Profile {
    lock: File,
    token: String,
    created: u64,
    path: PathBuf,
    backup: PathBuf,
    recovery: PathBuf,
    original: Vec<u8>,
    modified: Vec<u8>,
    hash: String,
}
impl Profile {
    fn prepare(
        lock_path: &Path,
        path: &Path,
        backup: &Path,
        expected_token: Option<&str>,
    ) -> Result<Self, String> {
        let mut lock = OpenOptions::new()
            .read(true)
            .share_mode(3)
            .open(lock_path)
            .map_err(|e| e.to_string())?;
        let created = self_creation()?;
        let token = validate_owner(&read_owner(&mut lock)?, created, expected_token)?;
        let original = fs::read(path).map_err(|e| e.to_string())?;
        let text = std::str::from_utf8(&original).map_err(|e| e.to_string())?;
        let modified = replace_option(
            &replace_option(text, "DeviceID", "140")?,
            "AllowSWRenderer",
            "1",
        )?
        .into_bytes();
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(backup)
            .map_err(|e| format!("create backup {}: {e}", backup.display()))?;
        file.write_all(&original)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        if fs::read(backup).map_err(|e| e.to_string())? != original {
            return Err("backup bytes differ from original; apply refused".into());
        }
        let hash = original_hash(&original);
        let recovery = recovery_path(lock_path);
        persist_recovery(
            &recovery,
            &serde_json::json!({
                "version":1, "options":path, "backup":backup,
                "original_sha256":hash, "original_bytes":original.len(),
                "pid":std::process::id(), "pid_created_filetime":created,
                "lock_token":token
            }),
        )?;
        println!("PROFILE ORIGINAL sha256={hash} backup={}", backup.display());
        Ok(Self {
            lock,
            token,
            created,
            path: path.into(),
            backup: backup.into(),
            recovery,
            original,
            modified,
            hash,
        })
    }
    fn validate(&mut self) -> Result<(), String> {
        if self_creation()? != self.created {
            return Err("invoking chain creation FILETIME changed".into());
        }
        validate_owner(
            &read_owner(&mut self.lock)?,
            self.created,
            Some(&self.token),
        )?;
        Ok(())
    }
    fn write_applied(&mut self) -> Result<(), String> {
        self.validate()?;
        if fs::read(&self.path).map_err(|e| e.to_string())? != self.original {
            return Err("AppOptions changed before apply; apply refused".into());
        }
        atomic_write(&self.path, &self.modified)?;
        if fs::read(&self.path).map_err(|e| e.to_string())? != self.modified {
            return Err("applied graphics bytes differ; restoration required".into());
        }
        Ok(())
    }
    #[cfg(test)]
    fn apply(lock_path: &Path, path: &Path, backup: &Path) -> Result<Self, String> {
        let mut profile = Self::prepare(lock_path, path, backup, None)?;
        profile.write_applied()?;
        Ok(profile)
    }
    fn restore(&mut self) -> Result<(), String> {
        self.validate()?;
        atomic_write(&self.path, &self.original)?;
        if fs::read(&self.path).map_err(|e| e.to_string())? != self.original {
            return Err("restored graphics bytes differ from original".into());
        }
        fs::remove_file(&self.recovery)
            .map_err(|e| format!("clear verified AppOptions recovery: {e}"))?;
        println!(
            "PROFILE RESTORED sha256={} verified=true before_lease_release=true backup={}",
            self.hash,
            self.backup.display()
        );
        Ok(())
    }
}
// Every process absence query shares the same absolute restoration deadline.
struct Bounded<'a, R> {
    inner: &'a mut R,
    deadline: Instant,
}
impl<R: Runner> Runner for Bounded<'_, R> {
    fn run(&mut self, mut call: Call) -> Result<Output, String> {
        let left = self
            .deadline
            .saturating_duration_since(Instant::now())
            .as_secs_f64();
        if left <= 0.0 {
            return Err("profile restoration deadline exhausted".into());
        }
        call.timeout = Some(call.timeout.unwrap_or(left).min(left));
        self.inner.run(call)
    }
    fn now(&mut self) -> f64 {
        self.inner.now()
    }
    fn monotonic(&mut self) -> f64 {
        self.inner.monotonic()
    }
    fn sleep(&mut self, s: f64) -> Result<(), String> {
        self.inner.sleep(s)
    }
}
trait ProfileRunner: Runner {
    fn game_running(&mut self, deadline: Instant) -> Result<bool, String>;
}
impl ProfileRunner for cc_live::process::RealRunner {
    fn game_running(&mut self, deadline: Instant) -> Result<bool, String> {
        if cc_live::chain::civ6_running(&mut Bounded {
            inner: self,
            deadline,
        })? {
            return Ok(true);
        }
        // Confirm absence through the native enumeration as well. Failed or
        // incomplete enumeration cannot authorize an AppOptions write.
        use windows_sys::Win32::{
            Foundation::{CloseHandle, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
            System::Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
        };
        struct Snapshot(windows_sys::Win32::Foundation::HANDLE);
        impl Drop for Snapshot {
            fn drop(&mut self) {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
        let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!(
                "native game absence snapshot: {}",
                std::io::Error::last_os_error()
            ));
        }
        let snapshot = Snapshot(handle);
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = unsafe { Process32FirstW(snapshot.0, &mut entry) };
        loop {
            if found == 0 {
                let error = std::io::Error::last_os_error();
                return if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                    Ok(false)
                } else {
                    Err(format!("native game absence enumeration: {error}"))
                };
            }
            if Instant::now() >= deadline {
                return Err("game absence deadline exhausted".into());
            }
            let len = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .ok_or("invalid native process image")?;
            let image = String::from_utf16(&entry.szExeFile[..len]).map_err(|e| e.to_string())?;
            if cc_live::chain::CIV_EXECUTABLES
                .iter()
                .any(|name| image.eq_ignore_ascii_case(name))
            {
                return Ok(true);
            }
            found = unsafe { Process32NextW(snapshot.0, &mut entry) };
        }
    }
}
struct Scoped<R: ProfileRunner> {
    inner: R,
    profile: Option<Profile>,
    paths: Paths,
    options: PathBuf,
    backup_dir: PathBuf,
    restore_deadline: Option<Instant>,
    start_token: Option<String>,
}
impl<R: ProfileRunner> Scoped<R> {
    fn restore_if_closed(&mut self, deadline: Instant) -> Result<bool, String> {
        if self.profile.is_none() {
            return Ok(true);
        }
        if self.inner.game_running(deadline)? {
            return Ok(false);
        }
        if Instant::now() >= deadline {
            return Err("profile restoration deadline exhausted".into());
        }
        self.profile.as_mut().unwrap().restore()?;
        // Drop the extension's lease read pin only after byte verification.
        drop(self.profile.take());
        cc_live::process::finish_external_cleanup();
        Ok(true)
    }
    fn restore_before_release(&mut self, remaining_seconds: f64) -> Result<(), String> {
        if self.profile.is_none() {
            return Ok(());
        }
        let window = if remaining_seconds.is_finite() {
            remaining_seconds.clamp(0.0, 90.0)
        } else {
            0.0
        };
        let end = Instant::now() + Duration::from_secs_f64(window);
        let deadline = (*self.restore_deadline.get_or_insert(end)).min(end);
        self.restore_deadline = Some(deadline);
        restore_until_deadline(
            || self.restore_if_closed(deadline),
            deadline,
            RESTORE_INTERVAL,
        )
    }
    fn pending(&self, reason: &str) {
        if let Some(profile) = &self.profile {
            eprintln!(
                "PROFILE RESTORATION PENDING sha256={} backup={} original_bytes={} reason={reason}",
                profile.hash,
                profile.backup.display(),
                profile.original.len()
            );
        }
    }
}
impl<R: ProfileRunner> Runner for Scoped<R> {
    fn start_scenario(
        &mut self,
        text: &str,
        token: Option<&str>,
        remaining_seconds: f64,
    ) -> Result<(), String> {
        self.start_token = Some(
            token
                .filter(|s| !s.is_empty())
                .ok_or("START lacks the chain's standard acquisition token")?
                .to_owned(),
        );
        let result = self.print_with_budget(text, remaining_seconds);
        self.start_token = None;
        result
    }
    fn run(&mut self, call: Call) -> Result<Output, String> {
        self.inner.run(call)
    }
    fn prepare_process_queries(&mut self) -> Result<(), String> {
        self.inner.prepare_process_queries()
    }
    fn process_identity(
        &mut self,
        pid: u32,
        timeout: f64,
    ) -> Result<Option<cc_live::process::ProcessIdentity>, String> {
        self.inner.process_identity(pid, timeout)
    }
    fn before_lease_release(&mut self, remaining_seconds: f64) -> Result<(), String> {
        let result = self.restore_before_release(remaining_seconds);
        if let Err(error) = &result {
            self.pending(error);
        }
        result?;
        self.inner.before_lease_release(remaining_seconds)
    }
    fn now(&mut self) -> f64 {
        self.inner.now()
    }
    fn monotonic(&mut self) -> f64 {
        self.inner.monotonic()
    }
    fn sleep(&mut self, s: f64) -> Result<(), String> {
        self.inner.sleep(s)
    }
    fn error(&mut self, text: &str) -> Result<(), String> {
        self.inner.error(text)
    }
    fn print(&mut self, text: &str) -> Result<(), String> {
        self.print_with_budget(text, RESTORE_TIMEOUT.as_secs_f64())
    }
    fn print_with_budget(&mut self, text: &str, remaining_seconds: f64) -> Result<(), String> {
        let budget = if remaining_seconds.is_finite() {
            remaining_seconds.clamp(0.0, 90.0)
        } else {
            0.0
        };
        let boundary_deadline = Instant::now() + Duration::from_secs_f64(budget);
        if text.starts_with("START ") {
            let expected_token = self
                .start_token
                .as_deref()
                .ok_or("START was not issued by the standard acquisition boundary")?;
            if !text.ends_with(" civ6=closed-before-prepare")
                || self.profile.is_some()
                || self.inner.game_running(boundary_deadline)?
            {
                return Err(
                    "software rendering probe requires an absent game under the standard lease"
                        .into(),
                );
            }
            let backup = self
                .backup_dir
                .join(format!("warp-options-{}.backup", unique_suffix()));
            // The standard lease and the confirmed game absence span recovery.
            // Failure leaves the old receipt intact and prevents driver launch.
            recover_profile(
                &self.paths.here.join("civ6.lock"),
                &self.options,
                expected_token,
            )?;
            let profile = Profile::prepare(
                &self.paths.here.join("civ6.lock"),
                &self.options,
                &backup,
                Some(expected_token),
            )?;
            // Arm recovery BEFORE the first write, including unknown outcomes.
            cc_live::process::begin_external_cleanup();
            self.profile = Some(profile);
            self.restore_deadline = None;
            if self.inner.game_running(boundary_deadline)? {
                return Err("game appeared before software renderer apply".into());
            }
            if Instant::now() >= boundary_deadline {
                return Err("profile apply deadline exhausted".into());
            }
            self.profile.as_mut().unwrap().write_applied()?;
            self.inner.print("PROFILE APPLY DeviceID=140 AllowSWRenderer=1; actual Renderer.log must confirm WARP")?;
        }
        if text.starts_with("CIV6_CLOSE outcome=") && self.profile.is_some() {
            let deadline =
                (*self.restore_deadline.get_or_insert(boundary_deadline)).min(boundary_deadline);
            self.restore_deadline = Some(deadline);
            // absent/graceful/forced exits share this path, without taskkill.
            // Keep transient failures for retry at the common pre-release hook.
            if let Err(error) = self.restore_if_closed(deadline) {
                self.pending(&error);
            }
        }
        self.inner.print(text)
    }
}
fn restore_until_deadline(
    mut attempt: impl FnMut() -> Result<bool, String>,
    deadline: Instant,
    interval: Duration,
) -> Result<(), String> {
    loop {
        let failure = match attempt() {
            Ok(true) => return Ok(()),
            Ok(false) => "owned game has not closed".to_owned(),
            Err(error) => error,
        };
        if Instant::now() >= deadline {
            return Err(failure);
        }
        std::thread::sleep(interval.min(deadline.saturating_duration_since(Instant::now())));
    }
}
#[cfg(test)]
fn restore_until(
    attempt: impl FnMut() -> Result<bool, String>,
    timeout: Duration,
    interval: Duration,
) -> Result<(), String> {
    restore_until_deadline(attempt, Instant::now() + timeout, interval)
}
fn main() {
    cc_live::process::install_interrupt_handler();
    let paths = Paths::default();
    let options = paths
        .profile
        .join("AppData/Local/Firaxis Games/Sid Meier's Civilization VI/AppOptions.txt");
    let mut runner = Scoped {
        inner: cc_live::process::RealRunner,
        profile: None,
        paths: paths.clone(),
        options,
        backup_dir: paths.here.clone(),
        restore_deadline: None,
        start_token: None,
    };
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = cc_live::chain::cli(&mut runner, &paths, &args);
    // Retry belongs to pre-release cleanup: never add another 90s after CLI.
    if runner.profile.is_some() {
        let reason = result
            .as_ref()
            .err()
            .map(String::as_str)
            .unwrap_or("chain returned before verified restoration; lease release incomplete");
        runner.pending(reason);
        std::process::exit(2);
    }
    let code = match result {
        Ok(code) => cc_live::chain::python_cli_exit(code),
        Err(error) => {
            eprintln!("{error}");
            1
        }
    };
    std::process::exit(code);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_preserves_other_settings_and_rejects_ambiguous_keys() {
        let text =
            "Version 15\r\n; gpu\r\nDeviceID 5056\r\nAllowSWRenderer 0\r\nPlayIntroVideo 0\r\n";
        let expected =
            "Version 15\r\n; gpu\r\nDeviceID 140\r\nAllowSWRenderer 1\r\nPlayIntroVideo 0\r\n";
        assert_eq!(
            replace_option(
                &replace_option(text, "DeviceID", "140").unwrap(),
                "AllowSWRenderer",
                "1"
            )
            .unwrap(),
            expected
        );
        assert!(replace_option("DeviceID 1\nDeviceID 2\n", "DeviceID", "140").is_err());
        assert!(replace_option("Version 15\n", "DeviceID", "140").is_err());
    }
    #[test]
    fn actual_windows_lease_blocks_delete_and_restores_exact_options() {
        let root = PathBuf::from(format!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/test-{}"),
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let lock = root.join("civ6.lock");
        let options = root.join("AppOptions.txt");
        let backup = root.join("backup");
        let original = b"Version 15\r\nDeviceID 5056\r\nAllowSWRenderer 0\r\n";
        fs::write(&options, original).unwrap();
        fs::write(&lock,serde_json::to_vec(&serde_json::json!({"pid":std::process::id(),"chain_pid":std::process::id(),"live_chain":true,"pid_created_filetime":cc_live::chain::process_created_filetime(std::process::id()).unwrap().unwrap(),"lock_token":"owned-fixture"})).unwrap()).unwrap();
        let mut profile = Profile::apply(&lock, &options, &backup).unwrap();
        assert!(fs::remove_file(&lock).is_err());
        assert_eq!(
            fs::read(&options).unwrap(),
            b"Version 15\r\nDeviceID 140\r\nAllowSWRenderer 1\r\n"
        );
        profile.restore().unwrap();
        drop(profile);
        assert_eq!(fs::read(&options).unwrap(), original);
        fs::remove_file(&lock).unwrap();
        fs::remove_file(&options).unwrap();
        fs::remove_file(&backup).unwrap();
        fs::remove_dir(&root).unwrap();
    }
    #[test]
    fn transient_native_restore_failure_retries_without_losing_the_profile() {
        let root = PathBuf::from(format!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/retry-test-{}"),
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let lock = root.join("civ6.lock");
        let options = root.join("AppOptions.txt");
        let backup = root.join("backup");
        let original = b"DeviceID 5056\r\nAllowSWRenderer 0\r\n";
        fs::write(&options, original).unwrap();
        fs::write(&lock,serde_json::to_vec(&serde_json::json!({"pid":std::process::id(),"chain_pid":std::process::id(),"live_chain":true,"pid_created_filetime":cc_live::chain::process_created_filetime(std::process::id()).unwrap().unwrap(),"lock_token":"retry-fixture"})).unwrap()).unwrap();
        let mut profile = Profile::apply(&lock, &options, &backup).unwrap();
        let mut blocker = Some(
            OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&options)
                .unwrap(),
        );
        let mut attempts = 0;
        restore_until(
            || {
                attempts += 1;
                let result = profile.restore();
                if attempts == 1 {
                    assert!(result.is_err(), "native locked file did not reject restore");
                    blocker.take();
                }
                result.map(|_| true)
            },
            Duration::from_secs(1),
            Duration::from_millis(1),
        )
        .unwrap();
        assert_eq!(attempts, 2);
        assert_eq!(fs::read(&options).unwrap(), original);
        drop(profile);
        fs::remove_file(&lock).unwrap();
        fs::remove_file(&options).unwrap();
        fs::remove_file(&backup).unwrap();
        fs::remove_dir(&root).unwrap();
    }
}
