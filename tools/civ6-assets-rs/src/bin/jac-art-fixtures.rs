//! Capture cooked package name-table prefixes, without distributing texture payloads.
use anyhow::Context;
use civ6_assets_rs::*;
use serde_json::json;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
fn capture(
    path: &Path,
    names: &BTreeSet<String>,
    output: &Path,
) -> anyhow::Result<serde_json::Value> {
    let file = fs::File::open(path)?;
    let map = unsafe { memmap2::MmapOptions::new().map(&file)? };
    let mut end = 1024;
    for name in names {
        let pos = map
            .windows(name.len())
            .position(|w| w == name.as_bytes())
            .with_context(|| format!("{} missing {name}", path.display()))?;
        end = end.max(pos + name.len() + 1);
    }
    end = end.min(map.len());
    write(output, &map[..end])?;
    Ok(
        json!({"source_file":path.file_name().unwrap().to_string_lossy(),"source_sha256":sha256(&map),"source_bytes":map.len(),"captured_prefix_bytes":end,"prefix_sha256":sha256(&map[..end]),"textures":names}),
    )
}
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    anyhow::ensure!(
        args.len() == 2,
        "usage: jac-art-fixtures <completed-build> <fixture-output>"
    );
    let build = PathBuf::from(&args[0]);
    let out = PathBuf::from(&args[1]);
    let mut records = vec![];
    for package in [
        "JAC_CustomCivs_Icons",
        "JAC_CustomCivs_Leaders",
        "JAC_CustomCivs_Fallback",
    ] {
        let xlp = xml(build.join("XLPs").join(format!("{package}.xlp")))?;
        let names = elements(xlp.get_child("m_Entries").context("xlp entries")?)
            .map(|e| attr(e.get_child("m_EntryID").unwrap(), "text").to_owned())
            .collect::<BTreeSet<_>>();
        let path = build
            .parent()
            .context("build parent")?
            .join("mod/Platforms/Windows/BLPs")
            .join(format!("{package}.blp"));
        records.push(capture(
            &path,
            &names,
            &out.join(format!("{package}.blp.prefix")),
        )?);
    }
    let base = read_json(out.join("base-icons.json"))?;
    let names = base["atlases"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|a| arr(a).iter())
        .map(|r| {
            Path::new(s(&r["Filename"]))
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    records.push(capture(
        &generate::game_dir().join("Base/Platforms/Windows/BLPs/UI/Icons.blp"),
        &names,
        &out.join("Icons.blp.prefix"),
    )?);
    write_json(
        out.join("cooked-packages.json"),
        &json!({"format":"verbatim prefix of real SDK BLP output; no texture payload","packages":records}),
    )?;
    Ok(())
}
