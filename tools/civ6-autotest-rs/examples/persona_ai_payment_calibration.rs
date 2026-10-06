//! Emits bounded driver scenarios and read-only UI evidence. Never launches a
//! game, installs the candidate, sends an AI operation, or fabricates a receipt.
#[path = "support/ai_payment_calibration.rs"]
mod calibration;
use std::{fs::OpenOptions, io::Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 3 && args[0] == "bundle" {
        let owner = args[1].parse()?;
        let plans = calibration::bundle(owner)?;
        for (_, plan) in &plans {
            civ6_autotest_rs::scenario::validate_scenario(&plan.scenario())?;
        }
        let directory = std::path::Path::new(&args[2]);
        std::fs::create_dir(directory)?;
        let mut entries = Vec::new();
        for (name, plan) in plans {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join(format!("{name}.json")))?;
            writeln!(file, "{}", serde_json::to_string_pretty(&plan.scenario())?)?;
            entries.push(serde_json::json!({"file":format!("{name}.json"),"requirements":plan.requirements()}));
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("requirements.json"))?;
        writeln!(
            file,
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"schema":"todo1466-calibration-bundle-v1","automatically_executable":false,"driver_scenario":false,"segments":entries,"parent_controls_initial_setup_and_save_reload":true})
            )?
        )?;
        println!("9 finite scenario segments written; parent supplies verified setup/registration/save/load boundaries; live=0");
        return Ok(());
    }
    if args.len() == 3 && args[0] == "compare-reload" {
        let before = serde_json::from_slice(&std::fs::read(&args[1])?)?;
        let after = serde_json::from_slice(&std::fs::read(&args[2])?)?;
        println!("{}", calibration::compare_reload(&before, &after)?);
        return Ok(());
    }
    if args.len() != 5 {
        return Err("usage: persona_ai_payment_calibration <step|hold|reload|unjoined> <AI owner> <cost 2|3> <paid before> <new output.json>".into());
    }
    let plan = calibration::Plan::new(
        &args[0],
        args[1].parse()?,
        args[2].parse()?,
        args[3].parse()?,
    )?;
    let scenario = plan.scenario();
    civ6_autotest_rs::scenario::validate_scenario(&scenario)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[4])?;
    writeln!(file, "{}", serde_json::to_string_pretty(&scenario)?)?;
    // Printed manifest is separate from the executable driver's action schema.
    println!("{}", serde_json::to_string_pretty(&plan.requirements())?);
    Ok(())
}

#[cfg(test)]
#[path = "support/ai_payment_calibration_tests.rs"]
mod tests;
