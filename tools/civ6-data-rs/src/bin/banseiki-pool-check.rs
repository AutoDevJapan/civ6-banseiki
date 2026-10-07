use anyhow::{bail, Result};
use civ6_data_rs::{pool, Repository};
use std::path::PathBuf;

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("Usage: banseiki-pool-check [catalogue.json]\nChecks the first proposal; does not register or unlock game characters.");
        return Ok(());
    }
    if args.len() > 1
        || args
            .first()
            .is_some_and(|a| a.to_string_lossy().starts_with('-'))
    {
        bail!("Usage: banseiki-pool-check [catalogue.json]");
    }
    let path = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| Repository::checked_in().path(pool::DATA_PATH));
    let data = serde_json::from_str(&civ6_data_rs::read_utf8(&path)?)?;
    pool::validate(&data)?;
    println!("万世紀プール: 候補100人・史実20人・新規80人。形式検査成功。方向確認待ち（ゲームへの登録は未実装）。");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
