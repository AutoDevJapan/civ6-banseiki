//! Offline fixed-Core operation registry and serialized queue audit.
#[path = "../ai_operation/operation_audit.rs"]
mod operation_audit;

fn audit_path(args: &[std::ffi::OsString]) -> Result<serde_json::Value, String> {
    let [path] = args else {
        return Err("usage: ai-operation-domain-audit <GameCore DLL>".into());
    };
    let bytes = std::fs::read(path).map_err(|e| format!("read Core image: {e}"))?;
    operation_audit::audit(&bytes)
}
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match audit_path(&args) {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_arguments_are_rejected_before_file_read() {
        for args in [vec![], vec!["missing".into(), "--enable".into()]] {
            assert!(audit_path(&args).unwrap_err().starts_with("usage:"));
        }
    }
}
