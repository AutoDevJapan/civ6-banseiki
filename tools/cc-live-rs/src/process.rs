use std::{
    collections::BTreeMap,
    io::Read,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const CREATE_NO_WINDOW: u32 = 0x08000000;
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static CLEANING: AtomicBool = AtomicBool::new(false);
static CLEANUP_DONE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub timeout: Option<f64>,
    pub merge_stderr: bool,
    pub creationflags: u32,
    pub kill_tree: bool,
}
impl Call {
    pub fn new(args: Vec<String>) -> Self {
        Self {
            args,
            env: BTreeMap::new(),
            timeout: None,
            merge_stderr: false,
            creationflags: CREATE_NO_WINDOW,
            kill_tree: false,
        }
    }
    pub fn timeout(mut self, seconds: f64) -> Self {
        self.timeout = Some(seconds);
        self
    }
    pub fn env(mut self, key: &str, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
    pub fn merged(mut self) -> Self {
        self.merge_stderr = true;
        self
    }
    /// Contain a build and all descendants until it completes or is cancelled.
    pub fn managed(mut self) -> Self {
        self.kill_tree = true;
        self
    }
}
#[derive(Clone, Debug, Default)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// All external commands (including gh/network) and waits are injectable. Tests never launch a game or call an API.
pub trait Runner {
    fn run(&mut self, call: Call) -> Result<Output, String>;
    fn now(&mut self) -> f64;
    fn monotonic(&mut self) -> f64 {
        self.now()
    }
    fn sleep(&mut self, seconds: f64) -> Result<(), String>;
    fn print(&mut self, text: &str) -> Result<(), String> {
        use std::io::Write;
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{text}")
            .and_then(|_| stdout.flush())
            .map_err(|e| e.to_string())
    }
    fn error(&mut self, text: &str) -> Result<(), String> {
        use std::io::Write;
        writeln!(std::io::stderr().lock(), "{text}").map_err(|e| e.to_string())
    }
}
pub struct RealRunner;

pub fn install_interrupt_handler() {
    #[cfg(windows)]
    unsafe {
        unsafe extern "system" fn handler(event: u32) -> i32 {
            if matches!(event, 0 | 1 | 2 | 5 | 6) {
                if !CLEANING.load(Ordering::SeqCst) {
                    INTERRUPTED.store(true, Ordering::SeqCst);
                }
                if matches!(event, 2 | 5 | 6) {
                    let deadline = Instant::now() + Duration::from_secs(4);
                    while !CLEANUP_DONE.load(Ordering::SeqCst) && Instant::now() < deadline {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
                1
            } else {
                0
            }
        }
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(handler), 1);
    }
}
/// Cleanup must still run after Ctrl+C; no new scenario is started once interruption is observed.
pub fn clear_interrupt_for_cleanup() {
    CLEANING.store(true, Ordering::SeqCst);
    INTERRUPTED.store(false, Ordering::SeqCst);
}
pub fn finish_cleanup() {
    CLEANUP_DONE.store(true, Ordering::SeqCst);
    CLEANING.store(false, Ordering::SeqCst);
}
fn interrupted() -> Result<(), String> {
    if INTERRUPTED.load(Ordering::SeqCst) {
        Err("interrupted".into())
    } else {
        Ok(())
    }
}

impl Runner for RealRunner {
    fn run(&mut self, call: Call) -> Result<Output, String> {
        interrupted()?;
        let mut cmd = Command::new(call.args.first().ok_or("empty command")?);
        cmd.args(&call.args[1..])
            .envs(&call.env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(call.creationflags);
        }
        // Using the same pipe preserves scenario.py's stdout/stderr line order.
        if call.merge_stderr {
            let (mut source, sink) = std::io::pipe().map_err(|e| e.to_string())?;
            cmd.stdout(Stdio::from(sink.try_clone().map_err(|e| e.to_string())?))
                .stderr(Stdio::from(sink));
            let (mut child, mut tree) =
                crate::process_tree::Tree::spawn(&mut cmd, call.creationflags, call.kill_tree)?;
            drop(cmd);
            let reader = thread::spawn(move || {
                let mut b = Vec::new();
                source.read_to_end(&mut b).map(|_| b)
            });
            let code = wait(&mut child, &mut tree, call.timeout)?;
            let bytes = reader
                .join()
                .map_err(|_| "stdout reader failed")?
                .map_err(|e| e.to_string())?;
            return Ok(Output {
                code,
                stdout: decode(&bytes),
                stderr: String::new(),
            });
        }
        cmd.stderr(Stdio::piped());
        let (mut child, mut tree) =
            crate::process_tree::Tree::spawn(&mut cmd, call.creationflags, call.kill_tree)?;
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let out = thread::spawn(move || {
            let mut b = Vec::new();
            stdout.read_to_end(&mut b).map(|_| b)
        });
        let err = thread::spawn(move || {
            let mut b = Vec::new();
            stderr.read_to_end(&mut b).map(|_| b)
        });
        let status = wait(&mut child, &mut tree, call.timeout)?;
        Ok(Output {
            code: status,
            stdout: decode(
                &out.join()
                    .map_err(|_| "stdout reader failed")?
                    .map_err(|e| e.to_string())?,
            ),
            stderr: decode(
                &err.join()
                    .map_err(|_| "stderr reader failed")?
                    .map_err(|e| e.to_string())?,
            ),
        })
    }
    fn now(&mut self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
    }
    fn monotonic(&mut self) -> f64 {
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_secs_f64()
    }
    fn sleep(&mut self, seconds: f64) -> Result<(), String> {
        let until = Instant::now() + Duration::from_secs_f64(seconds);
        while Instant::now() < until {
            interrupted()?;
            thread::sleep(
                until
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(100)),
            );
        }
        Ok(())
    }
}
fn decode(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}
fn wait(
    child: &mut std::process::Child,
    tree: &mut crate::process_tree::Tree,
    timeout: Option<f64>,
) -> Result<i32, String> {
    let start = Instant::now();
    loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            tree.stop(child)?;
            return Ok(s.code().unwrap_or(1));
        }
        if interrupted().is_err() || timeout.is_some_and(|t| start.elapsed().as_secs_f64() >= t) {
            tree.stop(child)?;
            let _ = child.kill();
            let _ = child.wait();
            return Err(if INTERRUPTED.load(Ordering::SeqCst) {
                "interrupted"
            } else {
                "command timed out"
            }
            .into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}
