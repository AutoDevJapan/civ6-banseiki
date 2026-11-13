fn render(text: &str) -> Result<String, Box<dyn std::error::Error>> {
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
    let appointment_begin = "-- BEGIN RUST GENERATED APPOINTMENT GLUE\n";
    let appointment_end = "-- END RUST GENERATED APPOINTMENT GLUE\n";
    let start = output
        .find(appointment_begin)
        .or_else(|| output.find("local function SyncAppointmentAdmission(forceClosed, onlyOwner)"))
        .ok_or("appointment publication missing")?;
    let finish = if output[start..].starts_with(appointment_begin) {
        start
            + output[start..]
                .find(appointment_end)
                .ok_or("appointment block end missing")?
            + appointment_end.len()
    } else {
        start
            + output[start..]
                .find("-- BEGIN RUST PERSONA EVENTS REPUBLISH")
                .or_else(|| output[start..].find(begin))
                .ok_or("paid block boundary missing")?
    };
    let output = format!(
        "{}{}{}",
        &output[..start],
        civ6_native_bridge_rs::appointment_glue::source(),
        &output[finish..]
    );
    // The file's last statement is the load entry owned by the paid block.
    let load = "\nInitializeProductionAdmission();\n";
    let output = match output.strip_suffix("\nSyncProductionAdmission(true);\n") {
        Some(body) => format!("{body}{load}"),
        None if output.ends_with(load) => output,
        None => return Err("Gameplay load entry missing".into()),
    };
    Ok(output)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os().nth(1).ok_or("pass repository root")?;
    let path = std::path::Path::new(&root)
        .join("poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua");
    let text = std::fs::read_to_string(&path)?.replace("\r\n", "\n");
    std::fs::write(path, render(&text)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markerless_parent_preserves_the_event_republication_block() {
        let legacy = format!("local function SyncAppointmentAdmission(forceClosed, onlyOwner)\n return;\nend\n{}{}",
            "-- BEGIN RUST PERSONA EVENTS REPUBLISH\nlocal function keep_synced_receipts() return 1474 end\n-- END RUST PERSONA EVENTS REPUBLISH\n",
            civ6_native_bridge_rs::paid_glue::source()) + "SyncProductionAdmission(true);\n";
        let result = render(&legacy).unwrap();
        assert!(result.ends_with("\nInitializeProductionAdmission();\n"));
        assert!(result.contains("local function keep_synced_receipts() return 1474 end"));
        assert_eq!(
            result
                .matches("-- BEGIN RUST PERSONA EVENTS REPUBLISH")
                .count(),
            1
        );
        assert!(
            result
                .find("-- END RUST GENERATED APPOINTMENT GLUE")
                .unwrap()
                < result
                    .find("-- BEGIN RUST PERSONA EVENTS REPUBLISH")
                    .unwrap()
        );
        assert_eq!(render(&result).unwrap(), result);
    }
}
