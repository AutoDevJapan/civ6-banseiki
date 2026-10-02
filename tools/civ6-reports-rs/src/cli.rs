use crate::{arbitration, common, research, scenario, tuning, Result};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    about = "Count AI arbitration requests, outcomes and actual costs from an uninterrupted 200-turn artifact."
)]
pub struct ArbitrationArgs {
    pub artifact: PathBuf,
    #[arg(long)]
    pub json: Option<PathBuf>,
}

#[derive(Debug, Parser)]
#[command(
    about = "Summarize only archived artifacts of a 200-turn tuning run; never connect to Civ6."
)]
pub struct TuningArgs {
    pub artifact: PathBuf,
    #[arg(long)]
    pub json: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum ResearchKind {
    #[default]
    Tech,
    Civic,
}

#[derive(Debug, Parser)]
#[command(about = "Summarize first observed research completion turns across major factions.")]
pub struct ResearchArgs {
    pub research_jsonl: PathBuf,
    #[arg(long, value_enum, default_value = "tech")]
    pub kind: ResearchKind,
}

#[derive(Debug, Parser)]
#[command(
    about = "Build a short, controlled gameplay test; its clock jump is never used by the 200-turn run."
)]
pub struct ScenarioArgs {
    #[arg(long, default_value_os_t = scenario::default_output())]
    pub output: PathBuf,
}

pub fn arbitration(args: ArbitrationArgs) -> Result<()> {
    let data = arbitration::summarize(&args.artifact)?;
    if let Some(path) = args.json {
        common::write_json(&path, &data)?;
    }
    common::print_text(&(arbitration::markdown(&data) + "\n"))
}

pub fn tuning(args: TuningArgs) -> Result<()> {
    let data = tuning::summarize(&args.artifact)?;
    if let Some(path) = args.json {
        common::write_json(&path, &data)?;
    }
    common::print_text(&(tuning::markdown(&data) + "\n"))
}

pub fn research(args: ResearchArgs) -> Result<()> {
    let kind = match args.kind {
        ResearchKind::Tech => "tech",
        ResearchKind::Civic => "civic",
    };
    common::print_text(&research::tsv(&research::summarize(
        &args.research_jsonl,
        kind,
    )?))
}

pub fn scenario(args: ScenarioArgs) -> Result<()> {
    common::write_json(&args.output, &scenario::scenario()?)
}
