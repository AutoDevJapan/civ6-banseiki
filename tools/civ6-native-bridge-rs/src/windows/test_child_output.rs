//! Bounded capture for exact, owned test children. File-backed streams cannot
//! fill a pipe or make output recovery wait for an inherited pipe writer.
use std::os::windows::process::CommandExt;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};
const LIMIT: u64 = 1024 * 1024;

struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
fn snapshot(reader: &mut File, len: u64) -> io::Result<Vec<u8>> {
    reader.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    reader.take(len.min(LIMIT + 1)).read_to_end(&mut bytes)?;
    Ok(bytes)
}
/// These exact fixture children do not launch background descendants. They
/// are stopped/reaped here on failure; callers never reuse a saved PID.
pub(super) fn run(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    let deadline = Instant::now() + timeout;
    // No temporary directory is created. These exclusively created files
    // retain their owning handles until the child is reaped and capture ends.
    let stdout = tempfile::NamedTempFile::new()?;
    let stderr = tempfile::NamedTempFile::new()?;
    let mut out_reader = stdout.reopen()?;
    let mut err_reader = stderr.reopen()?;
    let spawned = command
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(stdout.as_file().try_clone()?)
        .stderr(stderr.as_file().try_clone()?)
        .spawn();
    // Command retains its stdio configuration; release the parent's writers.
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = Owned(spawned?);
    let mut refusal = None;
    let status = loop {
        if stdout.as_file().metadata()?.len() > LIMIT || stderr.as_file().metadata()?.len() > LIMIT
        {
            refusal = Some("owned test child output limit");
            let _ = child.0.kill();
            break child.0.wait()?;
        }
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            refusal = Some("owned test child deadline");
            let _ = child.0.kill();
            break child.0.wait()?;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let out_len = stdout.as_file().metadata()?.len();
    let err_len = stderr.as_file().metadata()?.len();
    if out_len > LIMIT || err_len > LIMIT {
        refusal = Some("owned test child output limit");
    }
    let out = snapshot(&mut out_reader, out_len)?;
    let err = snapshot(&mut err_reader, err_len)?;
    drop(child);
    drop(out_reader);
    drop(err_reader);
    let cleanup = stdout.close().and_then(|_| stderr.close());
    let details = || {
        format!(
            "status={status}; stdout_bytes={out_len}; stderr_bytes={err_len}; \
        stdout_prefix={}; stderr_prefix={}",
            String::from_utf8_lossy(&out[..out.len().min(8192)]),
            String::from_utf8_lossy(&err[..err.len().min(8192)])
        )
    };
    if Instant::now() >= deadline && refusal.is_none() {
        refusal = Some("owned test child capture deadline");
    }
    if let Err(error) = cleanup {
        return Err(io::Error::other(format!(
            "owned capture cleanup: {error}; {}",
            details()
        )));
    }
    if let Some(reason) = refusal {
        return Err(io::Error::other(format!("{reason}; {}", details())));
    }
    Ok(Output {
        status,
        stdout: out,
        stderr: err,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn command(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.env("CIV6_CAPTURE_FIXTURE", mode).args([
            "--exact",
            "windows::test_child_output::tests::capture_fixture_child",
            "--nocapture",
            "--include-ignored",
        ]);
        command
    }
    #[test]
    #[ignore = "owned output-capture fixture subprocess only"]
    fn capture_fixture_child() {
        let Ok(mode) = std::env::var("CIV6_CAPTURE_FIXTURE") else {
            return;
        };
        match mode.as_str() {
            "large_failure" => {
                std::io::stdout()
                    .write_all(&vec![0xa5; 128 * 1024])
                    .unwrap();
                std::io::stderr()
                    .write_all(&vec![0x5a; 128 * 1024])
                    .unwrap();
                std::io::stderr()
                    .write_all(b"ORIGINAL_FAILURE_23\n")
                    .unwrap();
                std::io::stdout().flush().unwrap();
                std::io::stderr().flush().unwrap();
                std::process::exit(23);
            }
            "idle" => {
                println!("IDLE_READY");
                eprintln!("IDLE_ERROR_READY");
                std::thread::sleep(Duration::from_secs(30));
            }
            "excess" => {
                std::io::stdout()
                    .write_all(&vec![0xa5; 4 * 1024 * 1024])
                    .unwrap();
                std::io::stdout().flush().unwrap();
            }
            "small" => println!("SMALL_READY"),
            _ => panic!("unknown capture fixture mode"),
        }
    }
    #[test]
    fn both_large_streams_preserve_the_original_nonzero_exit_and_complete_output() {
        let output = run(&mut command("large_failure"), Duration::from_secs(10)).unwrap();
        assert_eq!(output.status.code(), Some(23));
        assert_eq!(
            output.stdout.iter().filter(|v| **v == 0xa5).count(),
            128 * 1024
        );
        assert_eq!(
            output.stderr.iter().filter(|v| **v == 0x5a).count(),
            128 * 1024
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("ORIGINAL_FAILURE_23"));
    }
    #[test]
    fn timeout_reaps_the_owned_child_keeps_diagnostics_and_allows_the_next_child() {
        let error = run(&mut command("idle"), Duration::from_secs(2))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("owned test child deadline")
                && error.contains("IDLE_READY")
                && error.contains("IDLE_ERROR_READY"),
            "{error}"
        );
        let next = run(&mut command("small"), Duration::from_secs(10)).unwrap();
        assert!(next.status.success());
        assert!(String::from_utf8_lossy(&next.stdout).contains("SMALL_READY"));
    }
    #[test]
    fn excess_output_is_bounded_and_never_reported_as_a_successful_fixture() {
        let error = run(&mut command("excess"), Duration::from_secs(10))
            .unwrap_err()
            .to_string();
        assert!(error.contains("owned test child output limit"));
        assert!(error.contains("stdout_bytes="));
        assert!(error.len() < 100_000);
    }
}
