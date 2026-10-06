//! Read-only primary-file audit. Never loads either image or opens a process.
#[path = "../ai_operation/task_audit.rs"]
mod task_audit;
fn main() {
    let paths: Vec<_> = std::env::args_os().skip(1).collect();
    let result = (|| -> Result<_, String> {
        if paths.len() != 2 {
            return Err(
                "usage: ai-task-lifecycle-audit <GameCore DLL> <CivilizationVI.exe>".into(),
            );
        }
        let core = std::fs::read(&paths[0]).map_err(|e| e.to_string())?;
        let host = std::fs::read(&paths[1]).map_err(|e| e.to_string())?;
        task_audit::audit(&core, &host)
    })();
    match result {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
