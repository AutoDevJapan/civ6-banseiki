//! Offline subprocess fixture; never a replacement for Codex or the Civ6 SDK.
use anyhow::{bail, ensure, Context, Result};
use civ6_assets_rs::{build_art, generate_portraits, write, write_json, xml};
use image::{Rgba, RgbaImage};
use serde_json::json;
use std::{io::Read, path::Path};

fn main() -> Result<()> {
    ensure!(
        std::env::var("JAC_ASSETS_TEST_PROCESS").as_deref() == Ok("1"),
        "This executable is only an offline test fixture"
    );
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--probe-cooker") {
        ensure!(
            args.len() == 5,
            "probe requires cooker, build, package, output"
        );
        return build_art::cook(
            Path::new(&args[1]),
            Path::new(&args[2]),
            &args[3],
            Path::new(&args[4]),
        );
    }
    let stage = match args.first().map(String::as_str) {
        Some("login") => "login",
        Some("exec") => "exec",
        Some("--no_mt") => "cooker",
        _ => bail!("unexpected fixture invocation: {args:?}"),
    };
    let mut prompt = String::new();
    if stage == "exec" {
        std::io::stdin().read_to_string(&mut prompt)?;
    }
    let record = json!({
        "args":args,
        "cwd":std::env::current_dir()?.to_string_lossy(),
        "environment_names":std::env::vars_os().map(|(k,_)|k.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        "prompt":prompt
    });
    write_json(format!("fixture-{stage}.json"), &record)?;
    match stage {
        "login" => match std::env::var("JAC_ASSETS_TEST_LOGIN").as_deref() {
            Ok("api-key") => eprintln!("Logged in using an API key"),
            Ok("none") => bail!("Not logged in"),
            _ => eprintln!("Logged in using ChatGPT"),
        },
        "exec" => {
            let key = "ODA_NOBUHIDE";
            ensure!(
                prompt == generate_portraits::prompt(key)?,
                "shared image prompt differs"
            );
            let cwd = std::env::current_dir()?;
            let cd = args
                .iter()
                .position(|a| a == "-C")
                .context("missing working directory")?;
            ensure!(
                Path::new(&args[cd + 1]).is_absolute(),
                "child -C must be absolute"
            );
            ensure!(
                Path::new(&args[cd + 1]).canonicalize()? == cwd.canonicalize()?,
                "child -C differs from cwd"
            );
            for (kind, width, height) in [
                ("portrait", 768, 768),
                ("figure", 768, 1024),
                ("background", 1024, 768),
            ] {
                let (width, height) = if kind == "background"
                    && std::env::var_os("JAC_ASSETS_TEST_BAD_IMAGE").is_some()
                {
                    (20, 20)
                } else {
                    (width, height)
                };
                RgbaImage::from_pixel(width, height, Rgba([60, 80, 100, 255]))
                    .save(generate_portraits::filename(key, kind))?;
            }
            println!("{{\"fixture\":\"offline PNGs\"}}");
        }
        "cooker" => {
            if std::env::var_os("JAC_ASSETS_TEST_COOKER_FAIL").is_some() {
                bail!("fixture cooker failure");
            }
            let output = args
                .iter()
                .position(|a| a == "--stewpot")
                .context("missing stewpot")?;
            let xlp = Path::new(args.last().context("missing XLP")?);
            ensure!(
                xlp.is_absolute() && xlp.is_file(),
                "relative XLP resolved incorrectly"
            );
            let source = xml(xlp)?;
            let names = civ6_assets_rs::elements(source.get_child("m_Entries").context("entries")?)
                .map(|e| civ6_assets_rs::attr(e.get_child("m_EntryID").unwrap(), "text"))
                .collect::<Vec<_>>();
            let mut bytes = vec![0; 1024];
            for name in names {
                bytes.extend(name.as_bytes());
                bytes.push(0);
            }
            write(
                Path::new(&args[output + 1]).join(xlp.with_extension("blp").file_name().unwrap()),
                bytes,
            )?;
            println!("offline package index created");
        }
        _ => unreachable!(),
    }
    Ok(())
}
