use crate::*;
use clap::{Parser, Subcommand};
#[derive(Parser)]
#[command(about = "Build 1534 Japan maps from pinned public-domain geography")]
pub struct GenerateArgs {
    #[arg(long,default_value_os_t=generate::official())]
    pub official: PathBuf,
    #[arg(long,default_value_os_t=root().join("tools/japan_map_scale/cache"))]
    pub cache: PathBuf,
    #[arg(long,default_value_os_t=generate::mod_dir().join("Maps"))]
    pub output: PathBuf,
}
pub fn japan_map_generate() -> Result<()> {
    let a = GenerateArgs::parse();
    generate::obtain_sources(&a.cache)?;
    let land = generate::read_shapes(&a.cache.join("ne_10m_land.zip"), "ne_10m_land", None)?;
    let japan = generate::read_shapes(
        &a.cache.join("ne_10m_admin_0_countries.zip"),
        "ne_10m_admin_0_countries",
        Some("JPN"),
    )?;
    let dem = generate::load_dem(&a.cache.join("etopo_2022_japan_3arcmin.csv"))?;
    let mut manifest = vec![];
    for km in constants::SCALES {
        let mut item = generate::generate_map(
            km,
            &land,
            &japan,
            &dem,
            &a.official,
            &a.output.join(format!("JAC_Japan_{km}km.Civ6Map")),
            false,
        )?;
        let settler = format!("JAC_Japan_{km}km_Settlers.Civ6Map");
        generate::generate_map(
            km,
            &land,
            &japan,
            &dem,
            &a.official,
            &a.output.join(&settler),
            true,
        )?;
        item["settler_file"] = serde_json::json!(settler);
        for suffix in ["", "_Settlers"] {
            write(
                a.output.join(format!("JAC_Japan_{km}km{suffix}.lua")),
                generate::STATIC_LUA,
            )?;
        }
        println!(
            "{km} km: {}x{}, {} land, {} bytes",
            n(&item["width"]),
            n(&item["height"]),
            n(&item["land_tiles"]),
            n(&item["bytes"])
        );
        manifest.push(item);
    }
    write_json(a.output.join("manifest.json"), &serde_json::json!(manifest))?;
    generate::write_resource_sites(
        &manifest,
        &a.output
            .parent()
            .context("map output parent")?
            .join("Scripts/JAC_ResourceSites.lua"),
    )?;
    if a.output == generate::mod_dir().join("Maps") {
        write_support::main_to(
            &a.output,
            &generate::mod_dir(),
            &root().join("tools/civ6-autotest/scenarios"),
        )?;
    }
    Ok(())
}
#[derive(Parser)]
pub struct ProsperityArgs {
    #[arg(long,default_value_os_t=generate::mod_dir().join("Maps"))]
    pub maps: PathBuf,
    #[arg(long,default_value_os_t=root().join("documents/spec/data/japan-1534-prosperity.json"))]
    pub output: PathBuf,
}
pub fn japan_map_prosperity() -> Result<()> {
    let a = ProsperityArgs::parse();
    write_json(&a.output, &prosperity::build(&a.maps)?)?;
    println!("wrote {}", a.output.display());
    Ok(())
}
#[derive(Parser)]
pub struct ProsperStartArgs {
    #[arg(long,default_value_os_t=generate::mod_dir().join("Maps"))]
    pub maps: PathBuf,
    #[arg(long,default_value_os_t=root().join("tools/civ6-autotest/scenarios/japan-prosperous-15km.json"))]
    pub scenario: PathBuf,
}
pub fn japan_map_prosper_start() -> Result<()> {
    let a = ProsperStartArgs::parse();
    for km in constants::SCALES {
        let report = prosper_start::apply_file(&prosper_start::map_path(&a.maps, km), &a.maps, km)?;
        if km == 15 {
            prosper_start::write_scenario(&report, &a.scenario)?;
        }
        println!("{km} km: {}", serde_json::to_string(&report)?);
    }
    Ok(())
}
#[derive(Parser)]
pub struct SupportArgs {
    #[command(subcommand)]
    pub command: SupportCommand,
}
#[derive(Subcommand)]
pub enum SupportCommand {
    /// Config, translations, Oda/Takeda scenarios, resources and prosperous start checks.
    Support {
        #[arg(long,default_value_os_t=generate::mod_dir().join("Maps"))]
        maps: PathBuf,
        #[arg(long,default_value_os_t=generate::mod_dir())]
        output: PathBuf,
        #[arg(long,default_value_os_t=root().join("tools/civ6-autotest/scenarios"))]
        scenarios: PathBuf,
    },
    /// Twenty additional factions, translations and artwork declarations (uses existing ArtDefs templates).
    CustomCivs {
        #[arg(long,default_value_os_t=root().join("poc/JAC_PoC_CustomCivs"))]
        output: PathBuf,
    },
    CityNames {
        #[arg(long,default_value_os_t=root().join("poc/JAC_PoC_CustomCivs"))]
        output: PathBuf,
    },
    Feuds {
        #[arg(long,default_value_os_t=generate::mod_dir().join("Data/JAC_Japan_Feuds.xml"))]
        output: PathBuf,
    },
}
pub fn japan_map_support() -> Result<()> {
    match SupportArgs::parse().command {
        SupportCommand::Support {
            maps,
            output,
            scenarios,
        } => write_support::main_to(&maps, &output, &scenarios),
        SupportCommand::CustomCivs { output } => write_custom_civs::main_to(&output),
        SupportCommand::CityNames { output } => write_city_names::main_to(&output),
        SupportCommand::Feuds { output } => xml_write(output, &write_feuds::build()?),
    }
}
#[derive(Parser)]
pub struct UnificationArgs {
    #[arg(long,default_value_os_t=generate::official())]
    pub official: PathBuf,
    #[arg(long,default_value_os_t=root().join("poc/JAC_Test_UnificationMap/Maps/JAC_Unification_Test.Civ6Map"))]
    pub output: PathBuf,
}
pub fn japan_map_unification() -> Result<()> {
    let a = UnificationArgs::parse();
    unification_test_map::build(&a.output, &a.official)?;
    println!(
        "{}: castles {:?}",
        unification_test_map::MAP_NAME,
        unification_test_map::castle_rows(&a.output)?
    );
    Ok(())
}
#[derive(Parser)]
pub struct CrestArgs {
    #[arg(long,default_value_os_t=build_art::originals().join("crests"))]
    pub output: PathBuf,
}
pub fn jac_draw_crests() -> Result<()> {
    draw_crests::generate(&CrestArgs::parse().output)?;
    Ok(())
}
#[derive(Parser)]
pub struct PortraitArgs {
    #[arg(long,num_args=1..)]
    pub only: Vec<String>,
    #[arg(long,default_value_os_t=build_art::originals())]
    pub originals: PathBuf,
    #[arg(long)]
    pub cli: Option<PathBuf>,
    #[arg(long)]
    pub print_prompt: bool,
}
pub fn jac_generate_portraits() -> Result<()> {
    let a = PortraitArgs::parse();
    let keys = if a.only.is_empty() {
        generate_portraits::SUBJECTS
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    } else {
        a.only
    };
    for key in &keys {
        generate_portraits::prompt(key)?;
    }
    if a.print_prompt {
        for key in &keys {
            println!("{}", generate_portraits::prompt(key)?);
        }
        return Ok(());
    }
    let cli = a.cli.map(Ok).unwrap_or_else(generate_portraits::find_cli)?;
    for key in keys {
        generate_portraits::generate(&key, &cli, &a.originals)?;
    }
    Ok(())
}
#[derive(Parser)]
pub struct BuildArgs {
    #[arg(long)]
    pub originals: Option<PathBuf>,
    #[arg(long)]
    pub growth_originals: Option<PathBuf>,
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long)]
    pub growth_mod: Option<PathBuf>,
    #[arg(long)]
    pub build: Option<PathBuf>,
    #[arg(long)]
    pub cooker: Option<PathBuf>,
    #[arg(long)]
    pub game: Option<PathBuf>,
}
pub fn jac_build_art() -> Result<()> {
    let a = BuildArgs::parse();
    let mut o = build_art::BuildOptions::default();
    if let Some(p) = a.originals {
        o.originals = p;
    }
    if let Some(p) = a.growth_originals {
        o.growth_originals = p;
    }
    if let Some(p) = a.output {
        o.mod_dir = p;
    }
    if let Some(p) = a.growth_mod {
        o.growth_mod = p;
    }
    if let Some(p) = a.build {
        o.build = p;
    }
    if let Some(p) = a.cooker {
        o.cooker = p;
    }
    if let Some(p) = a.game {
        o.game = p;
    }
    build_art::build(&o)
}
