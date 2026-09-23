//! The gwsim command-line interface.
//!
//! The argument types live in the library rather than the binary so that
//! integration tests, and later the relative checks of section 17.4, can
//! use the same code the `gwsim` binary runs.
//!
//! See `docs/DESIGN.md` sections 14 and 15.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

pub mod check;
pub mod check_opt;
pub mod compare;
pub mod coverage;
pub mod data;
pub mod describe;
pub mod evaluate;
pub mod info;
pub mod loading;
pub mod log;
pub mod optimise;
pub mod plan;
pub mod profile;
pub mod report;
pub mod results;
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
    /// Re-simulate one run of a result file and print its combat log.
    Log(LogArgs),
    /// Run two parties on the same seeds and compare them.
    Compare(CompareArgs),
    /// Run the relative checks of DESIGN §17.4 (RC1, RC2, RC4, RC5, RC6).
    Check(CheckArgs),
    /// Search for the best builds for a party's free slots.
    Optimise(Box<OptimiseArgs>),
    /// Manage account profiles: unlocked skills, heroes and title ranks.
    Profile(ProfileArgs),
}

/// `gwsim optimise` (T5.8.1).
#[derive(Debug, Args)]
pub struct OptimiseArgs {
    /// A party file, or the slug of a party in the data.
    #[arg(long)]
    pub party: String,

    /// The slots the search may change, by name. Repeatable.
    #[arg(long, value_name = "SLOT", required = true)]
    pub free: Vec<String>,

    /// A situation set's slug, or one situation (slug or file).
    #[arg(long, value_name = "ID")]
    pub situations: String,

    /// What the ranked list is sorted by: an objective, or a weighted sum
    /// such as `clear-time:1,deaths:20`. Default: clear-time.
    #[arg(long)]
    pub goal: Option<String>,

    /// The objectives of the trade-off frontier, comma-separated, from
    /// clear-time, deaths, damage-taken, energy-left and dp. Default:
    /// clear-time,deaths.
    #[arg(long)]
    pub objectives: Option<String>,

    /// The win rate every situation must reach.
    #[arg(long, default_value_t = 0.95)]
    pub threshold: f64,

    /// Judge the threshold on the weighted mean win rate rather than in
    /// every situation.
    #[arg(long)]
    pub aggregate_threshold: bool,

    /// Stop after this long: 90s, 5m, 1h. Default 5m unless --generations.
    #[arg(long)]
    pub budget: Option<String>,

    /// Stop after this many generations; with a fixed seed the run is then
    /// exactly reproducible.
    #[arg(long, value_name = "N")]
    pub generations: Option<usize>,

    /// Candidates per generation.
    #[arg(long, default_value_t = 64)]
    pub population: usize,

    /// Runs per situation at each evaluation stage, like 16,64,256.
    #[arg(long, value_name = "A,B,C")]
    pub stages: Option<String>,

    /// `evolutionary`, or `exhaustive` over a --pool file.
    #[arg(long, default_value = "evolutionary")]
    pub mode: String,

    /// The pool file for exhaustive mode.
    #[arg(long, value_name = "FILE")]
    pub pool: Option<PathBuf>,

    /// The most combinations exhaustive mode will run.
    #[arg(long, default_value_t = gwsim_opt::exhaustive::DEFAULT_LIMIT)]
    pub limit: usize,

    /// Parts of a free slot that stay fixed: `slot=1,2,gear,attributes,secondary`
    /// (bar positions counted from 1). Repeatable.
    #[arg(long, value_name = "SLOT=PARTS")]
    pub lock: Vec<String>,

    /// A free slot's role, instead of the inferred one: `slot=healer`.
    #[arg(long, value_name = "SLOT=ROLE")]
    pub role: Vec<String>,

    /// Only Reviewed skills are candidates.
    #[arg(long)]
    pub reviewed_only: bool,

    /// An account profile's name, from the user directory.
    #[arg(long)]
    pub profile: Option<String>,

    /// Where profiles live. Defaults to the platform's data directory.
    #[arg(long, value_name = "PATH")]
    pub user_dir: Option<PathBuf>,

    /// The master seed of the search and of every evaluation.
    #[arg(long, value_name = "S")]
    pub seed: Option<u64>,

    /// How many ranked builds to report.
    #[arg(long, default_value_t = 10)]
    pub top: usize,

    /// Write report 3 (JSON) here.
    #[arg(long, value_name = "PATH")]
    pub json: Option<PathBuf>,

    /// No progress lines on stderr.
    #[arg(long)]
    pub quiet: bool,

    /// Worker threads. The result does not depend on this.
    #[arg(long, value_name = "N")]
    pub threads: Option<usize>,

    /// Read data from this directory instead of `./data` or the built-in pack.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
}

/// `gwsim profile` (T5.7.4).
#[derive(Debug, Args)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommand,

    /// Where profiles live. Defaults to the platform's data directory, or
    /// `GWSIM_USER_DIR`.
    #[arg(long, value_name = "PATH", global = true)]
    pub user_dir: Option<PathBuf>,

    /// The data directory, for skill and hero names.
    #[arg(long, value_name = "PATH", global = true)]
    pub data_dir: Option<PathBuf>,
}

/// The subcommands of `gwsim profile`.
#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// List the profiles.
    List,
    /// Print a profile.
    Show { name: String },
    /// Create a profile with everything unlocked and every title maxed.
    New { name: String },
    /// Print a profile's file path, for editing by hand.
    Path { name: String },
    /// `set <name> title <track> <rank>`.
    Set {
        name: String,
        what: String,
        track: String,
        rank: u8,
    },
    /// `unlock <name> skill <slug…>`.
    Unlock {
        name: String,
        what: String,
        #[arg(required = true)]
        skills: Vec<String>,
    },
    /// `lock <name> skill <slug…>`.
    Lock {
        name: String,
        what: String,
        #[arg(required = true)]
        skills: Vec<String>,
    },
    /// `hero <name> add|remove <hero>`.
    Hero {
        name: String,
        action: String,
        hero: String,
    },
}

/// Which checks `gwsim check` runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CheckSuite {
    /// RC1, RC2 and RC6.
    Relative,
    /// Quick self-checks of the comparison machinery.
    Unit,
    /// Both.
    All,
}

/// How many runs each comparison uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CheckLevel {
    /// Fewer runs, for CI.
    Reduced,
    /// The full run counts.
    Full,
}

/// `gwsim check` (T4.10.2).
#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Which suite to run.
    #[arg(long, value_enum, default_value_t = CheckSuite::All)]
    pub suite: CheckSuite,

    /// Reduced run counts (CI) or full ones.
    #[arg(long, value_enum, default_value_t = CheckLevel::Reduced)]
    pub level: CheckLevel,

    /// Only these checks, by id (RC1, RC2, RC6, SELF, DET). Repeatable.
    #[arg(long, value_name = "ID")]
    pub only: Vec<String>,

    /// Where to write the JSON report. Defaults to `gwsim-check.json`.
    #[arg(long, value_name = "PATH")]
    pub report: Option<PathBuf>,

    /// Worker threads. The result does not depend on this.
    #[arg(long, value_name = "N")]
    pub threads: Option<usize>,

    /// Read data from this directory instead of `./data` or the built-in pack.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
}

/// `gwsim log` (T4.9.6).
#[derive(Debug, Args)]
pub struct LogArgs {
    /// A result file written by `gwsim evaluate --json PATH`.
    #[arg(long, value_name = "FILE")]
    pub result: PathBuf,

    /// The run, counted from 0 across the result's situations in order.
    #[arg(long, value_name = "N", default_value_t = 0)]
    pub run: usize,

    /// `text`, or `jsonl` for one JSON object per event.
    #[arg(long, value_name = "FORMAT", default_value = "text")]
    pub format: String,

    /// Include decisions, movement, energy changes and swing starts.
    #[arg(long)]
    pub verbose: bool,

    /// Log the run even though the data pack has changed since the result
    /// was written. The log may then disagree with the recorded summary.
    #[arg(long)]
    pub force: bool,

    /// Read data from this directory instead of `./data` or the built-in pack.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
}

/// `gwsim compare` (T4.9.8).
#[derive(Debug, Args)]
pub struct CompareArgs {
    /// The first party: a file, a party slug, or comma-separated skill codes.
    pub party_a: String,

    /// The second party, in the same forms.
    pub party_b: String,

    /// A situation slug or file, or a situation set's slug.
    #[arg(long, value_name = "ID")]
    pub situations: String,

    /// Run exactly this many seeds per situation. Without it, each party runs
    /// until stable and the shorter is extended to match.
    #[arg(long, value_name = "N")]
    pub runs: Option<usize>,

    /// The master seed, shared by both parties.
    #[arg(long, value_name = "S")]
    pub seed: Option<u64>,

    /// Worker threads. The result does not depend on this.
    #[arg(long, value_name = "N")]
    pub threads: Option<usize>,

    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,

    /// Read data from this directory instead of `./data` or the built-in pack.
    #[arg(long, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
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

/// `gwsim evaluate` (T3.10.6, T4.9.2).
#[derive(Debug, Args)]
pub struct EvaluateArgs {
    /// A party file, the slug of a party in the data, or up to eight
    /// comma-separated skill template codes (the first is the player).
    #[arg(long)]
    pub party: String,

    /// A situation file, or the slug of a situation in the data.
    #[arg(long, required_unless_present = "set", conflicts_with = "set")]
    pub situation: Option<String>,

    /// Every situation of a situation set, by slug, weighted as it says.
    #[arg(long, value_name = "SET")]
    pub set: Option<String>,

    /// Run exactly this many seeds per situation.
    #[arg(long, value_name = "N", conflicts_with = "auto")]
    pub runs: Option<usize>,

    /// Add runs until the result is stable (the default without --runs).
    #[arg(long)]
    pub auto: bool,

    /// The master seed.
    #[arg(long, value_name = "S")]
    pub seed: Option<u64>,

    /// Worker threads. The result does not depend on this.
    #[arg(long, value_name = "N")]
    pub threads: Option<usize>,

    /// Print the first run's combat log instead, as `text` or `json`.
    #[arg(long, value_name = "FORMAT")]
    pub log: Option<String>,

    /// Write the result file (docs/result-schema.md) to PATH, or print it
    /// instead of the text report when no PATH is given.
    #[arg(long, value_name = "PATH", num_args = 0..=1, default_missing_value = "-")]
    pub json: Option<PathBuf>,

    /// Add report 4: contributions per slot and skill, interrupts landed,
    /// and the energy timeline.
    #[arg(long)]
    pub breakdown: bool,

    /// Refuse unless every skill on the party's bars is Reviewed.
    #[arg(long)]
    pub reviewed_only: bool,

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

    /// Print as JSON; the same as `--format json`, for consistency with the
    /// other commands.
    #[arg(long, conflicts_with = "format")]
    pub json: bool,
}

impl ValidateArgs {
    /// The format asked for, by either flag.
    pub fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            self.format
        }
    }
}

/// How a command prints its findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Grouped by file, for a person to read.
    Text,
    /// One array of objects, for a tool to read.
    Json,
}
