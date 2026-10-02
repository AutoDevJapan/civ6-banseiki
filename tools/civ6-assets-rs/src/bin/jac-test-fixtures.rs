//! Refresh the portable, offline test data from pinned public geography and the installed SDK.
use civ6_assets_rs::*;
use serde_json::json;
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf};
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    anyhow::ensure!(
        args.len() == 2,
        "usage: jac-test-fixtures <pinned-cache> <fixture-output>"
    );
    let cache = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    generate::obtain_sources(&cache)?;
    let land = generate::read_shapes(&cache.join("ne_10m_land.zip"), "ne_10m_land", None)?;
    let japan = generate::read_shapes(
        &cache.join("ne_10m_admin_0_countries.zip"),
        "ne_10m_admin_0_countries",
        Some("JPN"),
    )?;
    let dem = generate::load_dem(&cache.join("etopo_2022_japan_3arcmin.csv"))?;
    let geography = json!({"sources":generate::SOURCES,"land":land,"japan":japan,"dem":dem});
    fs::create_dir_all(&output)?;
    let file = fs::File::create(output.join("geography.zip"))?;
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file(
        "geography.json",
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )?;
    zip.write_all(&serde_json::to_vec(&geography)?)?;
    zip.finish()?;
    let file = fs::File::create(output.join("source-cache.zip"))?;
    let mut zip = zip::ZipWriter::new(file);
    for (name, _, _) in generate::SOURCES {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )?;
        std::io::copy(&mut fs::File::open(cache.join(name))?, &mut zip)?;
    }
    zip.finish()?;
    let source = generate::official();
    let db =
        rusqlite::Connection::open_with_flags(&source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let schema=db.prepare("SELECT sql FROM sqlite_master WHERE type IN ('table','index') AND sql IS NOT NULL ORDER BY type DESC,name")?.query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let schema_map = db
        .prepare("SELECT name,sql FROM sqlite_master WHERE type='table'")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
    let xp2 = generate::game_dir().join("DLC/Expansion2/Maps/EarthMaps/WorldMapHuge_XP2.Civ6Map");
    let xp2 =
        rusqlite::Connection::open_with_flags(xp2, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let xp2_schema = xp2
        .prepare("SELECT name,sql FROM sqlite_master WHERE type='table'")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
    let game = generate::game_dir();
    let mut excerpts = serde_json::Map::new();
    for (path, tables) in [
        (
            "Base/Assets/Gameplay/Data/Features.xml",
            vec!["Feature_ValidTerrains"],
        ),
        (
            "Base/Assets/Gameplay/Data/GlobalParameters.xml",
            vec!["GlobalParameters"],
        ),
        (
            "DLC/Expansion2/Data/Expansion1_Modifiers.xml",
            vec!["DynamicModifiers"],
        ),
        (
            "DLC/Expansion2/Data/Expansion2_GlobalParameters.xml",
            vec!["GlobalParameters"],
        ),
        (
            "DLC/Expansion2/Data/Expansion2_Icons_Civilizations.xml",
            vec!["IconTextureAtlases"],
        ),
    ] {
        let root = xml(game.join(path))?;
        let mut e = xmltree::Element::new("GameInfo");
        for table in tables {
            if let Some(t) = root.get_child(table) {
                push(&mut e, t.clone());
            }
        }
        let mut raw = vec![];
        e.write(&mut raw)?;
        excerpts.insert(path.into(), json!(String::from_utf8(raw)?));
    }
    let rules = generate::official_resource_rules(&source)?;
    let colors = build_art::house_colors(&game, &root().join("poc/JAC_PoC_CustomCivs"))?;
    write_json(
        output.join("official.json"),
        &json!({"schema":schema,"schema_map":schema_map,"xp2_schema":xp2_schema,"rules":rules,"colors":colors,"excerpts":excerpts}),
    )?;
    fs::copy(
        game.join("Base/Assets/UI/Colors/PlayerStandardColors.xml"),
        output.join("PlayerStandardColors.xml"),
    )?;
    println!(
        "Portable fixtures: {} land rings, {} Japan rings",
        arr(&geography["land"]).len(),
        arr(&geography["japan"]).len()
    );
    Ok(())
}
