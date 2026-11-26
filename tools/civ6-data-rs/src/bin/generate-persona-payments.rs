fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os().nth(1).ok_or("pass repository root")?;
    let file =
        std::path::Path::new(&root).join("poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona.sql");
    let source = std::fs::read_to_string(&file)?.replace("\r\n", "\n");
    std::fs::write(file, civ6_data_rs::persona_payments::regenerate(&source)?)?;
    Ok(())
}
