fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("usage: check-paid-artifact FILE PID CREATED_FILETIME".into());
    }
    let text = std::fs::read_to_string(&args[0])?;
    let decisions =
        civ6_native_bridge_rs::paid_artifact::validate(&text, args[1].parse()?, args[2].parse()?)?;
    println!(
        "{}",
        serde_json::json!({"capture_valid":true,"assignment_decisions":decisions,"product_acceptance":false})
    );
    Ok(())
}
