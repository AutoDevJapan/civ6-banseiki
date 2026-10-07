//! Offline explicit-source scene generator. It never opens a game process.
fn main() {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.len() != 6 {
            return Err("usage: civ6-bootstrap-scene PREFIX.json RUNTIME.dll SHA256 BOOTSTRAP.dll SHA256 PHYSICAL_SAVE_NAME".into());
        }
        let prefix = serde_json::from_slice(&std::fs::read(&args[0])?)?;
        let scene = civ6_autotest_rs::bootstrap_scenario::scene(
            &prefix, &args[1], &args[2], &args[3], &args[4], &args[5],
        )?;
        println!("{}", serde_json::to_string_pretty(&scene)?);
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
