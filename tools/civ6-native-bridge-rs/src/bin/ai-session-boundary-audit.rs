//! Offline primary-file audit. Never loads either image or opens a process.
#[path = "../ai_operation/session_audit.rs"]
mod session_audit;

fn main() {
    let paths: Vec<_> = std::env::args_os().skip(1).collect();
    let result = (|| {
        if paths.len() != 2 {
            return Err(
                "usage: ai-session-boundary-audit <GameCore DLL> <CivilizationVI.exe>".to_string(),
            );
        }
        let core = std::fs::read(&paths[0]).map_err(|e| format!("read Core: {e}"))?;
        let host = std::fs::read(&paths[1]).map_err(|e| format!("read Host: {e}"))?;
        session_audit::audit(&core, &host)
    })();
    match result {
        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    }
}
