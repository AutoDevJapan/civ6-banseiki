use crate::{arbitration, common, research, scenario, tuning, Result};
use clap::builder::{OsStringValueParser, TypedValueParser};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

fn path_value_parser() -> impl TypedValueParser<Value = PathBuf> {
    // Match pathlib.Path('') while preserving every nonempty native path.
    OsStringValueParser::new().map(|value| {
        if value.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(value)
        }
    })
}

#[derive(Debug, Parser)]
#[command(
    about = "Count AI arbitration requests, outcomes and actual costs from an uninterrupted 200-turn artifact.",
    infer_long_args = true,
    args_override_self = true
)]
pub struct ArbitrationArgs {
    #[arg(value_parser = path_value_parser())]
    pub artifact: PathBuf,
    #[arg(long, value_parser = path_value_parser())]
    pub json: Option<PathBuf>,
}

#[derive(Debug, Parser)]
#[command(
    about = "Summarize only archived artifacts of a 200-turn tuning run; never connect to Civ6.",
    infer_long_args = true,
    args_override_self = true
)]
pub struct TuningArgs {
    #[arg(value_parser = path_value_parser())]
    pub artifact: PathBuf,
    #[arg(long, value_parser = path_value_parser())]
    pub json: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum ResearchKind {
    #[default]
    Tech,
    Civic,
}

#[derive(Debug, Parser)]
#[command(
    about = "Summarize first observed research completion turns across major factions.",
    infer_long_args = true,
    args_override_self = true
)]
pub struct ResearchArgs {
    #[arg(value_parser = path_value_parser())]
    pub research_jsonl: PathBuf,
    #[arg(long, value_enum, default_value = "tech")]
    pub kind: ResearchKind,
}

#[derive(Debug, Parser)]
#[command(
    about = "Build a short, controlled gameplay test; its clock jump is never used by the 200-turn run.",
    infer_long_args = true,
    args_override_self = true
)]
pub struct ScenarioArgs {
    #[arg(
        long,
        value_parser = path_value_parser(),
        help = "Output JSON path (default: the selected repository's scenarios/court-arbitration-ai.json)"
    )]
    pub output: Option<PathBuf>,
    #[arg(
        long,
        value_parser = path_value_parser(),
        help = "Repository to read (default: discover from the executable, then the working directory)"
    )]
    pub repo: Option<PathBuf>,
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
    let repository = scenario::repository_root(args.repo.as_deref())?;
    let output = args
        .output
        .unwrap_or_else(|| scenario::output_in_repository(&repository));
    common::write_json(&output, &scenario::scenario_in_repository(&repository)?)
}
