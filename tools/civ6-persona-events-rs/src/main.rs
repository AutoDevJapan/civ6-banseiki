use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    match args.first().map(String::as_str) {
        Some("embed") if args.len() == 1 => {
            let file = root.join(civ6_persona_events_rs::GAMEPLAY);
            let source = fs::read_to_string(&file)?.replace("\r\n", "\n");
            fs::write(file, civ6_persona_events_rs::embed(&source)?)?;
        }
        Some("generate") if args.len() == 3 => {
            let template = serde_json::from_slice(&fs::read(&args[1])?)?;
            let scenario = civ6_persona_events_rs::scenario::build(&template)?;
            fs::write(&args[2], serde_json::to_string_pretty(&scenario)?)?;
        }
        Some("generate-sp-runtime") if args.len() == 3 => {
            let template = serde_json::from_slice(&fs::read(&args[1])?)?;
            let scenario = civ6_persona_events_rs::sp_runtime::build(&template)?;
            fs::write(&args[2], serde_json::to_string_pretty(&scenario)?)?;
        }
        _ => {
            return Err(
                "usage: civ6-persona-events-rs embed | generate TEMPLATE.json OUTPUT.json | generate-sp-runtime TEMPLATE.json OUTPUT.json".into(),
            )
        }
    }
    Ok(())
}
