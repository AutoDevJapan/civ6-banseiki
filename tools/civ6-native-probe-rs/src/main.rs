#[cfg(all(windows, target_arch = "x86_64"))]
use civ6_native_probe_rs::MAX_SECONDS;
#[cfg(all(windows, target_arch = "x86_64"))]
use std::path::PathBuf;

#[cfg(all(windows, target_arch = "x86_64"))]
fn run(args: &[String]) -> Result<(), String> {
    use civ6_native_probe_rs::windows::{run_observer, ObserverConfig};
    if args.first().map(String::as_str) != Some("observe") {
        return Err(usage().into());
    }
    let keys = [
        "--pid",
        "--created",
        "--exe",
        "--chain-pid",
        "--lock",
        "--controller-pid",
        "--controller-created",
        "--output",
        "--max-seconds",
    ];
    if args.len() % 2 == 0
        || args
            .iter()
            .skip(1)
            .step_by(2)
            .any(|a| !keys.contains(&a.as_str()))
    {
        return Err(usage().into());
    }
    let get = |key: &str| -> Result<String, String> {
        let positions: Vec<_> = args
            .iter()
            .enumerate()
            .filter(|(_, x)| x.as_str() == key)
            .collect();
        if positions.len() != 1 {
            return Err(format!("exactly one {key} is required"));
        }
        args.get(positions[0].0 + 1)
            .cloned()
            .ok_or_else(|| format!("missing value for {key}"))
    };
    let pid = get("--pid")?.parse::<u32>().map_err(|_| "invalid --pid")?;
    let created = get("--created")?
        .parse::<u64>()
        .map_err(|_| "invalid --created")?;
    let chain_pid = get("--chain-pid")?
        .parse::<u32>()
        .map_err(|_| "invalid --chain-pid")?;
    let controller_pid = get("--controller-pid")?
        .parse::<u32>()
        .map_err(|_| "invalid --controller-pid")?;
    let controller_created = get("--controller-created")?
        .parse::<u64>()
        .map_err(|_| "invalid --controller-created")?;
    let seconds = get("--max-seconds")?
        .parse::<u64>()
        .map_err(|_| "invalid --max-seconds")?;
    if seconds == 0 || seconds > MAX_SECONDS {
        return Err("--max-seconds must be 1..300".into());
    }
    let config = ObserverConfig {
        pid,
        created,
        exe: PathBuf::from(get("--exe")?),
        chain_pid,
        lock: PathBuf::from(get("--lock")?),
        controller_pid,
        controller_created,
        output: PathBuf::from(get("--output")?),
        seconds,
    };
    run_observer(config)
}

#[cfg(not(all(windows, target_arch = "x86_64")))]
fn run(_: &[String]) -> Result<(), String> {
    Err("civ6-native-probe requires Windows x86-64; runtime observation is unavailable on this platform".into())
}

fn usage() -> &'static str {
    "usage (Windows x86-64 only): civ6-native-probe observe --pid PID --created FILETIME --exe FULL_PATH --chain-pid PID --lock civ6.lock --controller-pid PID --controller-created FILETIME --output trace.jsonl --max-seconds 1..300"
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{}", usage());
        return;
    }
    if let Err(error) = run(&args) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
