fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os().nth(1).ok_or("pass repository root")?;
    let path = std::path::Path::new(&root)
        .join("poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua");
    let text = std::fs::read_to_string(&path)?.replace("\r\n", "\n");
    let begin = "-- BEGIN RUST GENERATED PAID ASSIGNMENT GLUE";
    let end = "-- END RUST GENERATED PAID ASSIGNMENT GLUE\n";
    let generated = civ6_native_bridge_rs::paid_glue::source();
    let output = if let Some(start) = text.find(begin) {
        let finish = start
            + text[start..]
                .find(end)
                .ok_or("missing generated block end")?
            + end.len();
        format!("{}{}{}", &text[..start], generated, &text[finish..])
    } else {
        text.replacen(
            "local function SyncProductionAdmission(forceClosed, onlyOwner)",
            "local function SyncAppointmentAdmission(forceClosed, onlyOwner)",
            1,
        )
        .replacen(
            "local function Join(owner, source, personality, trait)",
            &(generated + "local function Join(owner, source, personality, trait)"),
            1,
        )
    };
    std::fs::write(path, output)?;
    Ok(())
}
