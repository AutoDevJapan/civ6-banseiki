// Included inside native_bridge::platform to reuse its pinned process handle,
// child transport, stop/reap and failure cleanup rather than clone a controller.
fn bootstrap_terminal(path: &Path, pid: u32, created: u64) -> Result<()> {
    let bytes = std::fs::read(path.with_extension("bootstrap.bin"))?;
    civ6_bootstrap_rs::protocol::trace(&bytes, pid, created).map_err(Error::input)?;
    // Do not rely on the controller receipt alone: the stopped runtime prefix
    // needs both preserved whole captures of its separate property log.
    let log = std::fs::read(path.with_extension("property-log-first.bin"))?;
    if log != std::fs::read(path.with_extension("property-log-second.bin"))? {
        return Err(Error::input("bootstrap property log captures differ"));
    }
    let native = std::fs::read(path.with_extension("runtime.bin"))?;
    civ6_bootstrap_rs::protocol::native_trace(&native, Some(&log), pid, created)
        .map_err(Error::input)?;
    let text = std::fs::read_to_string(path)?;
    let row: Value = serde_json::from_str(
        text.lines()
            .last()
            .ok_or_else(|| Error::input("bootstrap terminal missing"))?,
    )?;
    if row["type"] != "bootstrap_stop"
        || row["pid"] != pid
        || row["created_filetime"] != created
        || row["observation_stopped"] != true
        || row["resident_code_retained"] != true
    {
        return Err(Error::input("bootstrap stop identity/receipt differs"));
    }
    Ok(())
}
impl Session {
    pub fn start_bootstrap(
        pid: u32,
        started: f64,
        output: &Path,
        step: &Value,
        close_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
    ) -> Result<Self> {
        let (created, target) = pinned_created(pid, started)?;
        let name = step["name"]
            .as_str()
            .ok_or_else(|| Error::scenario("bootstrap name missing"))?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err(Error::scenario(
                "bootstrap artifact name must be a simple identifier",
            ));
        }
        let artifact = output.join(format!("{name}.jsonl"));
        let request = serde_json::json!({"schema":"todo880-bootstrap-controller-v2","pid":pid,"created_filetime":created,"max_seconds":step["max_seconds"],"runtime_dll":step["runtime_dll"],"runtime_sha256":step["runtime_sha256"],"bootstrap_dll":step["bootstrap_dll"],"bootstrap_sha256":step["bootstrap_sha256"],"output":artifact});
        civ6_bootstrap_rs::protocol::Request::parse(&request).map_err(Error::scenario)?;
        if artifact.exists() {
            return Err(Error::scenario("bootstrap artifact exists"));
        }
        let request_path = artifact.with_extension("request.json");
        serde_json::to_writer(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&request_path)?,
            &request,
        )?;
        let executable = std::env::current_exe()?.with_file_name("civ6-startup-diagnostics.exe");
        // Keep the exact helper and both explicit DLLs immutable while the
        // shared Session owns startup, stop and failure recovery.
        use std::os::windows::fs::OpenOptionsExt;
        let parsed =
            civ6_bootstrap_rs::protocol::Request::parse(&request).map_err(Error::scenario)?;
        let mut binary_pins = Vec::new();
        for (path, expected) in [
            (&executable, None),
            (&parsed.runtime, Some(parsed.runtime_sha.as_str())),
            (&parsed.bootstrap, Some(parsed.bootstrap_sha.as_str())),
        ] {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(path)?;
            let actual = format!("{:x}", Sha256::digest(std::fs::read(path)?));
            if let Some(expected) = expected {
                verify_override_hash(Some(expected), &actual)?;
            }
            binary_pins.push(file);
        }
        let child = Command::new(executable)
            .arg("bootstrap")
            .arg("--request")
            .arg(request_path)
            .env(
                "CIV6_AUTOTEST_CONTROLLER_PID",
                std::process::id().to_string(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
        Self::from_child(
            child,
            artifact,
            pid,
            created,
            started,
            target,
            BridgeMode::ProductionAdmission,
            close_game,
            false,
            binary_pins,
            true,
        )
    }
    pub fn bootstrap_healthy(&self) -> Result<()> {
        if !self.bootstrap
            || self.finished
            || unsafe { WaitForSingleObject(self.target.0, 0) } != WAIT_TIMEOUT
            || unsafe { WaitForSingleObject(self.child.as_raw_handle() as HANDLE, 0) }
                != WAIT_TIMEOUT
        {
            return Err(Error::input("bootstrap controller or pinned game stopped"));
        }
        Ok(())
    }
}
