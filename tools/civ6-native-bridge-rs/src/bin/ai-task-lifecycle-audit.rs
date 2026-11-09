//! Read-only primary-file audit. Never loads either image or opens a process.
//! Binary-only module: no candidate feature or runtime registration is enabled.
#[path = "../ai_operation/task_audit.rs"]
mod task_audit;
#[derive(Debug, PartialEq, Eq)]
enum Slice {
    Legacy,
    Worker,
    Outer,
    Consumer,
}
fn worker_selector(paths: &[std::ffi::OsString]) -> Result<Slice, String> {
    match paths {
        [_, _] => Ok(Slice::Legacy),
        [_, _, selector] if selector == "--worker-payload" => Ok(Slice::Worker),
        [_, _, selector] if selector == "--outer-frames" => Ok(Slice::Outer),
        [_, _, selector] if selector == "--queue-consumer" => Ok(Slice::Consumer),
        _ => Err(
            "usage: ai-task-lifecycle-audit <GameCore DLL> <CivilizationVI.exe> [--worker-payload|--outer-frames|--queue-consumer]"
                .into(),
        ),
    }
}
fn main() {
    let paths: Vec<_> = std::env::args_os().skip(1).collect();
    let result = (|| -> Result<_, String> {
        let worker = worker_selector(&paths)?;
        let core = std::fs::read(&paths[0]).map_err(|e| format!("read Core image: {e}"))?;
        let host = std::fs::read(&paths[1]).map_err(|e| format!("read host image: {e}"))?;
        match worker {
            Slice::Worker => task_audit::audit_worker_payload(&core, &host),
            Slice::Outer => task_audit::audit_outer_frames(&core, &host),
            Slice::Consumer => task_audit::audit_queue_consumer(&core, &host),
            Slice::Legacy => task_audit::audit(&core, &host),
        }
    })();
    match result {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
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
    fn worker_selector_is_explicit_and_old_two_args_are_unchanged() {
        let base = vec!["core".into(), "host".into()];
        assert_eq!(worker_selector(&base).unwrap(), Slice::Legacy);
        let mut selected = base.clone();
        selected.push("--worker-payload".into());
        assert_eq!(worker_selector(&selected).unwrap(), Slice::Worker);
        let mut outer = base.clone();
        outer.push("--outer-frames".into());
        assert_eq!(worker_selector(&outer).unwrap(), Slice::Outer);
        let mut consumer = base.clone();
        consumer.push("--queue-consumer".into());
        assert_eq!(worker_selector(&consumer).unwrap(), Slice::Consumer);
        for unknown in [
            "worker-payload",
            "--root",
            "",
            "--worker-payload=true",
            "--outer-frames=true",
            "--queue-consumer=true",
        ] {
            let mut wrong = base.clone();
            wrong.push(unknown.into());
            assert!(worker_selector(&wrong).is_err());
        }
        selected.push("extra".into());
        assert!(worker_selector(&selected).is_err());
        outer.push("--worker-payload".into());
        assert!(worker_selector(&outer).is_err());
        consumer.push("--outer-frames".into());
        assert!(worker_selector(&consumer).is_err());
        assert!(worker_selector(&[]).is_err());
    }
}
