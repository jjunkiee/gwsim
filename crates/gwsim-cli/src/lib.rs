//! The gwsim command-line interface.
//!
//! The argument types live in the library rather than the binary so that
//! integration tests, and later the relative checks of section 17.4, can
//! use the same code the `gwsim` binary runs.
//!
//! See `docs/DESIGN.md` sections 14 and 15.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

pub mod coverage;
pub mod data;
pub mod describe;
pub mod evaluate;
pub mod info;
pub mod loading;
pub mod plan;
pub mod template;

/// Guild Wars Reforged PvE build simulator.
#[derive(Debug, Parser)]
#[command(
    name = "gwsim",
    version = env!("GWSIM_VERSION_STRING"),
    about = "Guild Wars Reforged PvE build simulator"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The subcommands of `gwsim`.
///
/// `evaluate` arrives with M0 (T3.10.6), `template` with T1.3.6, `check` and
/// `compare` with M1, and `optimise` with M2.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect and check the game data.
    Data(DataArgs),
    /// Read and write build template codes.
    Template(TemplateArgs),
    /// Run a party in a situation many times and report the results.
    Evaluate(EvaluateArgs),
    /// Print the priority plan a human slot would follow, as editable RON.
    Plan(PlanArgs),
}

/// `gwsim plan` (T4.6.2).
#[derive(Debug, Args)]
pub struct PlanArgs {
    /// A party file, or the slug of a party in the data.
    #[arg(long)]
    pub party: String,

    /// The slot, by name. Defaults to the first human slot.
    #[arg(long)]
    pub slot: Option<String>,

    /// Read data from this directory instead of `./data` or the built-in pack.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
}

/// `gwsim evaluate` (T3.10.6).
#[derive(Debug, Args)]
pub struct EvaluateArgs {
    /// A party file, or the slug of a party in the data.
    #[arg(long)]
    pub party: String,

    /// A situation file, or the slug of a situation in the data.
    #[arg(long)]
    pub situation: String,

    /// Run exactly this many seeds. Without it, runs are added until the
    /// result is stable.
    #[arg(long, value_name = "N")]
    pub runs: Option<usize>,

    /// The master seed.
    #[arg(long, value_name = "S")]
    pub seed: Option<u64>,

    /// Worker threads. The result does not depend on this.
    #[arg(long, value_name = "N")]
    pub threads: Option<usize>,

    /// Print one run's combat log instead, as `text` or `json`.
    #[arg(long, value_name = "FORMAT")]
    pub log: Option<String>,

    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,

    /// Read data from this directory instead of `./data` or the built-in pack.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
}

/// `gwsim template`.
#[derive(Debug, Args)]
pub struct TemplateArgs {
    #[command(subcommand)]
    pub command: TemplateCommand,
}

/// The subcommands of `gwsim template`.
#[derive(Debug, Subcommand)]
pub enum TemplateCommand {
    /// Read a build code and print what it contains.
    Decode(DecodeArgs),
    /// Write a template file out as a build code.
    Encode(EncodeArgs),
}

/// `gwsim template decode`.
#[derive(Debug, Args)]
pub struct DecodeArgs {
    /// The code, as pasted from the game or a build site.
    pub code: String,

    /// Print the result as JSON.
    #[arg(long)]
    pub json: bool,

    /// Where to look up skill names. Defaults to `./data`.
    ///
    /// Names are a nicety: a code decodes with no data loaded at all.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
}

/// `gwsim template encode`.
#[derive(Debug, Args)]
pub struct EncodeArgs {
    /// A RON file holding a `Build`, a `SkillTemplate` or an
    /// `EquipmentTemplate`.
    pub file: PathBuf,

    /// The data directory to read, for the rune and insignia template ids a
    /// build's equipment code needs. Defaults to `./data`.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
}

/// `gwsim data`.
#[derive(Debug, Args)]
pub struct DataArgs {
    #[command(subcommand)]
    pub command: DataCommand,
}

/// The subcommands of `gwsim data`.
#[derive(Debug, Subcommand)]
pub enum DataCommand {
    /// Check the data files and report every problem.
    Validate(ValidateArgs),
    /// Say what a skill does, in gwsim's own words.
    Describe(DescribeSkillArgs),
    /// Report how much of the game the data covers.
    Coverage(CoverageArgs),
    /// Say which data this binary is using.
    Info(InfoArgs),
}

/// `gwsim data info`.
#[derive(Debug, Args)]
pub struct InfoArgs {
    /// The data directory to read. Defaults to `./data`, then the embedded
    /// data.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,

    /// Where a person's own files live. Defaults to the platform's data
    /// directory, or `GWSIM_USER_DIR` when that is set.
    #[arg(long, value_name = "PATH")]
    pub user_dir: Option<PathBuf>,

    /// Print as JSON.
    #[arg(long)]
    pub json: bool,
}

/// `gwsim data coverage`.
#[derive(Debug, Args)]
pub struct CoverageArgs {
    /// Only this profession's row.
    #[arg(long, value_name = "NAME")]
    pub profession: Option<String>,

    /// Print as JSON.
    #[arg(long)]
    pub json: bool,

    /// The data directory to read. Defaults to `./data`, then the embedded
    /// data.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
}

/// `gwsim data describe`.
#[derive(Debug, Args)]
pub struct DescribeSkillArgs {
    /// A skill's slug, name or template id. Omit it to use the filters.
    pub skill: Option<String>,

    /// Evaluate scaled values at this attribute rank.
    #[arg(long, value_name = "N")]
    pub rank: Option<u8>,

    /// Evaluate title-scaled values at this title rank.
    #[arg(long, value_name = "N")]
    pub title_rank: Option<u8>,

    /// Only skills of this profession, such as `Mesmer`.
    ///
    /// Taken as a string rather than a typed enum so that the data crate
    /// does not have to depend on clap; `describe::run` parses it and says
    /// what the valid names are.
    #[arg(long, value_name = "NAME")]
    pub profession: Option<String>,

    /// Only skills at this review status: NumbersOnly, Draft or Reviewed.
    #[arg(long, value_name = "STATUS")]
    pub status: Option<String>,

    /// Every skill. Required to print the whole tree, so that a bare
    /// `describe` asks for what you want rather than dumping everything.
    #[arg(long, conflicts_with_all = ["skill", "profession", "status"])]
    pub all: bool,

    /// Print a markdown review table (T4.1.1) instead: name, wiki link, type
    /// and costs, generated text, roles and status.
    #[arg(long)]
    pub review_sheet: bool,

    /// Print as JSON.
    #[arg(long)]
    pub json: bool,

    /// The data directory to read. Defaults to `./data`, then the embedded
    /// data.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
}

/// `gwsim data validate`.
#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// The data directory to check.
    ///
    /// Defaults to `./data` when it exists, and otherwise to the data built
    /// into this binary. The report says which was used.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,

    /// How to print the report.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
}

/// How a command prints its findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Grouped by file, for a person to read.
    Text,
    /// One array of objects, for a tool to read.
    Json,
}
