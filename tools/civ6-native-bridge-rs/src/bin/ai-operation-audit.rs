//! Offline audit only: no process handles, DLL loading, hooks, or operation sender.
#[path = "../ai_operation/pinned.rs"]
mod pinned;
// The replay protocol deliberately has no production adapter or exported API.
#[cfg(windows)]
#[allow(dead_code)]
#[path = "../ai_operation/capture.rs"]
mod capture;
#[allow(dead_code)]
#[path = "../ai_operation/protocol.rs"]
mod protocol;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let result = match (args.next(), args.next()) {
        (Some(path), None) => std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| pinned::audit(&bytes)),
        _ => Err("usage: ai-operation-audit <read-only pinned GameCore DLL>".into()),
    };
    match result {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
