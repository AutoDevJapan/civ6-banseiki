use crate::*;
use anyhow::{bail, ensure};
use serde_json::json;
use std::{
    collections::BTreeMap,
    io::Write,
    process::{Command, Stdio},
    sync::LazyLock,
};
pub static SUBJECTS: LazyLock<Value> = LazyLock::new(|| {
    Value::Object(
        civ6_data_rs::asset_dependencies::art::SUBJECTS
            .into_iter()
            .map(|(key, name, age, clothing, scene)| {
                (key.to_owned(), json!([name, age, clothing, scene]))
            })
            .collect(),
    )
});
pub use civ6_data_rs::asset_dependencies::art::{filename, prompt};
pub fn validate(path: &Path, kind: &str, originals: &Path) -> Result<Value> {
    let raw = fs::read(path)?;
    let format = image::guess_format(&raw)?;
    ensure!(
        format == image::ImageFormat::Png,
        "{}: expected PNG, got {format:?}",
        path.display()
    );
    let (w, h) = image::load_from_memory_with_format(&raw, format)?.dimensions();
    match kind {
        "portrait" => ensure!(
            w >= 768 && h >= 768 && w.abs_diff(h) <= 16,
            "{}: portrait must be square and at least 768px",
            path.display()
        ),
        "figure" => ensure!(
            w >= 768 && h >= 1024 && h > w,
            "{}: figure must be tall and at least 768x1024",
            path.display()
        ),
        "background" => ensure!(
            w >= 1024 && h >= 768 && w > h,
            "{}: background must be landscape and at least 1024x768",
            path.display()
        ),
        _ => bail!("Unknown image kind: {kind}"),
    };
    Ok(
        json!({"output_file":path.strip_prefix(originals.parent().context("originals parent")?)?.to_string_lossy().replace('\\',"/"),"sha256":sha256(&raw),"pixels":[w,h]}),
    )
}
use image::GenericImageView;
pub fn is_api_key_env(name: &str) -> bool {
    let name: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    name.contains("APIKEY")
        || name.contains("APITOKEN")
        || [
            "OPENAIKEY",
            "AZUREOPENAIKEY",
            "ANTHROPICKEY",
            "GOOGLEKEY",
            "GEMINIKEY",
            "AWSACCESSKEYID",
            "AWSSECRETACCESSKEY",
        ]
        .contains(&name.as_str())
}
pub fn oauth_environment() -> BTreeMap<std::ffi::OsString, std::ffi::OsString> {
    std::env::vars_os()
        .filter(|(name, _)| !is_api_key_env(&name.to_string_lossy()))
        .collect()
}
fn command(cli: &Path, directory: &Path) -> Command {
    let mut c = Command::new(cli);
    c.current_dir(directory)
        .env_clear()
        .envs(oauth_environment());
    hidden(&mut c);
    c
}
pub fn find_cli() -> Result<PathBuf> {
    let paths = std::env::var_os("PATH").context("PATH missing")?;
    for path in std::env::split_paths(&paths) {
        for name in if cfg!(windows) {
            vec!["codex.cmd", "codex.exe"]
        } else {
            vec!["codex"]
        } {
            let path = path.join(name);
            if path.is_file() {
                return Ok(std::path::absolute(path)?);
            }
        }
    }
    bail!("Codex CLI unavailable")
}
pub fn generate(key: &str, cli: &Path, originals: &Path) -> Result<()> {
    let text = prompt(key)?;
    let originals = std::path::absolute(originals)?;
    let directory = originals.join(key.to_lowercase());
    fs::create_dir_all(&directory)?;
    let images: Vec<_> = ["portrait", "figure", "background"]
        .into_iter()
        .map(|kind| (kind, directory.join(filename(key, kind))))
        .collect();
    if images.iter().all(|(_, path)| path.is_file()) {
        for (kind, path) in images {
            validate(&path, kind, &originals)?;
        }
        println!("EXISTING {key}: three existing PNGs verified");
        return Ok(());
    }
    ensure!(
        !images.iter().any(|(_, path)| path.exists()),
        "{key}: partial existing output; inspect before retrying"
    );
    // Resolve relative executable paths before changing the child working directory.
    let cli = if cli.is_file() || cli.components().count() > 1 {
        std::path::absolute(cli)?
    } else {
        cli.to_owned()
    };
    let login = command(&cli, &directory)
        .args(["login", "status"])
        .output()?;
    let status = format!(
        "{}{}",
        String::from_utf8_lossy(&login.stdout),
        String::from_utf8_lossy(&login.stderr)
    );
    ensure!(
        login.status.success()
            && status.contains("ChatGPT")
            && !status.to_lowercase().contains("api key"),
        "Portrait generation requires Codex CLI ChatGPT OAuth login"
    );
    let started = chrono::Utc::now().to_rfc3339();
    println!("START {key} {started}");
    let mut child = command(&cli, &directory)
        .args([
            "exec",
            "--json",
            "--ignore-user-config",
            "--skip-git-repo-check",
            "-s",
            "danger-full-access",
            "-C",
        ])
        .arg(&directory)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .context("CLI stdin")?
        .write_all(text.as_bytes())?;
    let result = child.wait_with_output()?;
    let mut log = result.stdout;
    log.extend(result.stderr);
    write(directory.join("codex-cli-events.jsonl"), &log)?;
    ensure!(
        result.status.success(),
        "{key}: Codex CLI exit {}; inspect codex-cli-events.jsonl",
        result.status
    );
    let missing: Vec<_> = images
        .iter()
        .filter(|(_, p)| !p.is_file())
        .map(|(_, p)| p)
        .collect();
    ensure!(
        missing.is_empty(),
        "{key}: Codex CLI returned without generating {missing:?}; inspect codex-cli-events.jsonl"
    );
    let completed = chrono::Utc::now().to_rfc3339();
    let artifacts = images
        .iter()
        .map(|(kind, path)| validate(path, kind, &originals))
        .collect::<Result<Vec<_>>>()?;
    let log_path = originals
        .parent()
        .context("art store parent")?
        .join("generation-log.jsonl");
    let mut log = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(log_path)?;
    for artifact in artifacts {
        let mut row = artifact.as_object().unwrap().clone();
        row.extend(json!({"leader":key,"tool":"official Codex CLI built-in image_gen (ChatGPT OAuth)","started_utc":started,"completed_utc":completed,"prompt":text}).as_object().unwrap().clone());
        writeln!(log, "{}", serde_json::to_string(&row)?)?;
    }
    println!("OK {key} {completed}: 3 PNGs");
    Ok(())
}
