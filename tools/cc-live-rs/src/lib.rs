pub mod chain;
pub mod driver;
pub mod fairness;
pub mod process;
mod process_tree;
pub mod review;
pub mod ship;
pub use process::{Call, Output, Runner};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub here: PathBuf,
    pub wt: PathBuf,
    pub jobs: PathBuf,
    pub temp: PathBuf,
    pub profile: PathBuf,
    pub python: String,
}
impl Default for Paths {
    fn default() -> Self {
        Self {
            here: PathBuf::from("D:/CC/Tools/civ6-live"),
            wt: PathBuf::from(r"C:\Users\makhmeto\Desktop\dev\ADJ\civ6-banseiki-wt"),
            jobs: PathBuf::from("D:/CC/Tools/codex-jobs/jobs"),
            temp: std::env::var_os("TEMP")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir),
            profile: std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_default(),
            python: std::env::var("CC_LIVE_PYTHON").unwrap_or_else(|_| {
                r"C:\Users\makhmeto\AppData\Local\Programs\Python\Python312\python.exe".into()
            }),
        }
    }
}
impl Paths {
    pub fn live(&self) -> PathBuf {
        self.wt.join("live")
    }
}
pub fn path(p: impl AsRef<Path>) -> String {
    p.as_ref()
        .components()
        .collect::<PathBuf>()
        .display()
        .to_string()
}
pub fn read_text(p: impl AsRef<Path>) -> Result<String, String> {
    std::fs::read_to_string(p.as_ref())
        .map(|s| s.replace("\r\n", "\n").replace('\r', "\n"))
        .map_err(|e| format!("{}: {e}", p.as_ref().display()))
}
pub fn write_text(p: impl AsRef<Path>, s: &str) -> Result<(), String> {
    #[cfg(windows)]
    let s = s.replace('\n', "\r\n");
    std::fs::write(p, s).map_err(|e| e.to_string())
}
pub fn read_json(p: impl AsRef<Path>) -> Result<Value, String> {
    serde_json::from_str(&read_text(p)?).map_err(|e| e.to_string())
}
pub fn write_json(p: impl AsRef<Path>, data: &Value) -> Result<(), String> {
    std::fs::write(p, python_json(data)).map_err(|e| e.to_string())
}
/// Python json.dumps separators, UTF-8 and insertion order: existing state formats are retained.
pub fn python_json(v: &Value) -> String {
    match v {
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(python_json).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", serde_json::to_string(k).unwrap(), python_json(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => serde_json::to_string(v).unwrap(),
    }
}
pub fn tail(s: &str, n: usize) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    if chars.len() > n {
        chars.drain(..chars.len() - n);
    }
    chars.into_iter().collect()
}
pub fn head(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
pub fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}
