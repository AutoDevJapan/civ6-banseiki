use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    match args.first().map(String::as_str) {
        Some("generate-mp-hotseat" | "generate-mp-setup" | "generate-mp-setup-read") if args.len()==4 => {
            let normal=serde_json::from_slice(&fs::read(&args[1])?)?;
            let hotseat=serde_json::from_slice(&fs::read(&args[2])?)?;
            let scenario=if args[0]=="generate-mp-setup-read" {
                civ6_persona_events_rs::mp_setup_read::build(&root,&normal,&hotseat)?
            } else if args[0]=="generate-mp-setup" {
                civ6_persona_events_rs::mp_hotseat::build_setup_probe(&normal,&hotseat)?
            } else {civ6_persona_events_rs::mp_hotseat::build(&normal,&hotseat)?};
            fs::write(&args[3],serde_json::to_string_pretty(&scenario)?)?;
        }
        Some("embed") if args.len() == 1 || args.len() == 2 => {
            let root = args.get(1).map(PathBuf::from).unwrap_or(root);
            let file = root.join(civ6_persona_events_rs::GAMEPLAY);
            let source = fs::read_to_string(&file)?.replace("\r\n", "\n");
            fs::write(file, civ6_persona_events_rs::embed(&source)?)?;
            for (path, render) in [
                ("poc/JAC_PoC_LeaderPersona/UI/JAC_LeaderPersona_View.lua", civ6_persona_events_rs::price_routes::ui as fn(&str) -> Result<String, String>),
                ("poc/JAC_PoC_LeaderPersona/Shared/JAC_PersonaTreeRules.lua", civ6_persona_events_rs::price_routes::shared),
            ] {
                let file = root.join(path);
                let source = fs::read_to_string(&file)?.replace("\r\n", "\n");
                fs::write(file, render(&source)?)?;
            }
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
        Some("generate-sp-reassign") if args.len() == 4 || args.len() == 5 => {
            let template = serde_json::from_slice(&fs::read(&args[1])?)?;
            let turns = args.get(4).map(|n| n.parse()).transpose()?.unwrap_or(6);
            let input = if args[2] == "-" { None } else {
                let path = PathBuf::from(&args[2]);
                if !path.is_file() || fs::metadata(&path)?.len() == 0 || path.extension().and_then(|v| v.to_str()) != Some("Civ6Save") {
                    return Err("input must be an existing nonempty SP04 physical .Civ6Save; use - for a non-executable draft".into());
                }
                Some(path.file_stem().and_then(|v| v.to_str()).ok_or("input filename is not UTF-8")?.to_owned())
            };
            let scenario = civ6_persona_events_rs::sp_reassign::build(&template, input.as_deref(), turns)?;
            fs::write(&args[3], serde_json::to_string_pretty(&scenario)?)?;
        }
        Some("generate-sp-skill") if args.len() == 5 => {
            let template = serde_json::from_slice(&fs::read(&args[1])?)?;
            let (input, merit) = if args[2] == "-" && args[3] == "-" { (None,None) } else {
                let path=PathBuf::from(&args[2]);
                if !path.is_file() || fs::metadata(&path)?.len()==0 || path.extension().and_then(|v|v.to_str())!=Some("Civ6Save") {
                    return Err("skill input must be a nonempty successful reassignment .Civ6Save; draft uses - -".into());
                }
                (Some(path.file_stem().and_then(|v|v.to_str()).ok_or("invalid save filename")?.to_owned()),Some(args[3].parse::<u32>()?))
            };
            let scenario=civ6_persona_events_rs::sp_skill::build(&template,input.as_deref(),merit)?;
            fs::write(&args[4],serde_json::to_string_pretty(&scenario)?)?;
        }
        _ => {
            return Err(
                "usage: civ6-persona-events-rs generate-mp-hotseat NORMAL.json HOTSEAT.json OUTPUT.json | generate-mp-setup NORMAL.json HOTSEAT.json OUTPUT.json | generate-mp-setup-read NORMAL.json HOTSEAT.json OUTPUT.json | embed | generate TEMPLATE.json OUTPUT.json | generate-sp-runtime TEMPLATE.json OUTPUT.json | generate-sp-reassign TEMPLATE.json INPUT.Civ6Save|- OUTPUT.json [REAL_TURNS] | generate-sp-skill TEMPLATE.json INPUT.Civ6Save|- OBSERVED_MERIT|- OUTPUT.json".into(),
            )
        }
    }
    Ok(())
}
