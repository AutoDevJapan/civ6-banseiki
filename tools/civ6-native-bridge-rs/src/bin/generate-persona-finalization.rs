fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os().nth(1).ok_or("pass repository root")?;
    let path = std::path::Path::new(&root)
        .join("poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua");
    let text = std::fs::read_to_string(&path)?;
    let output = civ6_native_bridge_rs::persona_finalization::regenerate(&text)?;
    if output != text {
        std::fs::write(path, output)?;
    }
    Ok(())
}
