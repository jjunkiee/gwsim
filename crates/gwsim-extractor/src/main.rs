//! The wiki extractor: crawler, seeder and change report.
//!
//! A developer tool. It is **not shipped** with gwsim, because the data
//! it produces is committed to the repository instead (D8).
//!
//! See `docs/DESIGN.md` section 9 for the crawl rules EXT-1 to EXT-7, and
//! the README's contributor section for a worked example.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use gwsim_data::core::CoreData;
use gwsim_data::provenance::{Provenance, ReviewStatus};
use gwsim_data::skill_index::SkillIndexFile;
use gwsim_data::{DirSource, WikiTitle};
use gwsim_extractor::cache::PageCache;
use gwsim_extractor::client::{ClientOptions, Delay};
use gwsim_extractor::clock::{SystemClock, iso_date};
use gwsim_extractor::crawl::{self, CrawlOptions, Scope};
use gwsim_extractor::transport::UreqTransport;
use gwsim_extractor::{diff, discovery, foe, seed};

/// The gwsim wiki extractor (developer tool; not shipped).
#[derive(Debug, Parser)]
#[command(
    name = "gwsim-extract",
    version,
    about = "gwsim wiki extractor (developer tool; not shipped)"
)]
struct Cli {
    /// Where the page cache lives. It holds raw wiki HTML and is never
    /// committed.
    #[arg(
        long,
        global = true,
        default_value = ".cache/wiki",
        value_name = "PATH"
    )]
    cache_dir: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Refresh the page cache from the wiki, slowly and resumably.
    Crawl(CrawlArgs),
    /// Write data/skills/index.ron from the cached skill lists.
    Index(DataDirArgs),
    /// Write initial data files from the cache. Never overwrites a file.
    Seed(SeedArgs),
    /// Compare the cache with the committed data. Never writes data.
    Diff(DiffArgs),
    /// Print an area's foe roster from the cache.
    Roster {
        /// The area page title, such as "Vehtendi Valley".
        area: String,
    },
    /// Print a numbers-only table of parsed skills for hand-checking
    /// (T2.4.8), as markdown, and optionally as JSON.
    ParseCheck {
        /// The skill titles to include.
        #[arg(long, num_args = 1.., value_name = "TITLE", required = true)]
        only: Vec<String>,
        #[arg(long, value_name = "FILE")]
        json: Option<PathBuf>,
    },
    /// Compare a cached category's first page with the skill index.
    CategoryCheck {
        /// The category title, such as "Category:Fast Casting skills".
        category: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ScopeArg {
    Skills,
    Foes,
    Areas,
    All,
}

impl From<ScopeArg> for Scope {
    fn from(scope: ScopeArg) -> Self {
        match scope {
            ScopeArg::Skills => Scope::Skills,
            ScopeArg::Foes => Scope::Foes,
            ScopeArg::Areas => Scope::Areas,
            ScopeArg::All => Scope::All,
        }
    }
}

#[derive(Debug, Args)]
struct CrawlArgs {
    #[arg(long, value_enum, default_value_t = ScopeArg::All)]
    scope: ScopeArg,
    /// Fetch exactly these titles, skipping discovery.
    #[arg(long, num_args = 1.., value_name = "TITLE")]
    only: Vec<String>,
    /// Fetch only the discovery pages.
    #[arg(long)]
    discovery_only: bool,
    /// Seconds between requests. At least 2 (EXT-3).
    #[arg(long, default_value_t = 3.0, value_name = "SECS")]
    delay: f64,
    /// Skip pages checked within this many days.
    #[arg(long, value_name = "DAYS")]
    max_age: Option<f64>,
    /// List the URLs and fetch nothing.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct DataDirArgs {
    #[arg(long, default_value = "data", value_name = "PATH")]
    data_dir: PathBuf,
}

#[derive(Debug, Args)]
struct SeedArgs {
    #[arg(long, value_enum, default_value_t = ScopeArg::Skills)]
    scope: ScopeArg,
    /// The titles to seed. Required for foes.
    #[arg(long, num_args = 1.., value_name = "TITLE")]
    only: Vec<String>,
    #[arg(long, default_value = "data", value_name = "PATH")]
    data_dir: PathBuf,
    /// Print what would be created and write nothing.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct DiffArgs {
    #[arg(long, value_enum, default_value_t = ScopeArg::All)]
    scope: ScopeArg,
    #[arg(long, default_value = "data", value_name = "PATH")]
    data_dir: PathBuf,
    /// Write the markdown report here instead of printing it.
    #[arg(long, value_name = "FILE")]
    out: Option<PathBuf>,
    /// Also write the report as JSON.
    #[arg(long, value_name = "FILE")]
    json: Option<PathBuf>,
    /// Cached game-update pages to attach as context, by title.
    #[arg(long, num_args = 1.., value_name = "TITLE")]
    updates: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let cache = PageCache::new(&cli.cache_dir);
    let mut out = io::stdout().lock();
    let result = match cli.command {
        Command::Crawl(args) => run_crawl(args, &cache, &mut out),
        Command::Index(args) => run_index(args, &cache, &mut out),
        Command::Seed(args) => run_seed(args, &cache, &mut out),
        Command::Diff(args) => run_diff(args, &cache, &mut out),
        Command::Roster { area } => run_roster(&area, &cache, &mut out),
        Command::CategoryCheck { category } => run_category_check(&category, &cache, &mut out),
        Command::ParseCheck { only, json } => {
            run_parse_check(&only, json.as_deref(), &cache, &mut out)
        }
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(io::stderr(), "gwsim-extract: {error}");
            ExitCode::FAILURE
        }
    }
}

type Outcome = Result<ExitCode, Box<dyn std::error::Error>>;

fn run_crawl(args: CrawlArgs, cache: &PageCache, out: &mut dyn Write) -> Outcome {
    let delay = Delay::from_secs(args.delay)?;
    let options = CrawlOptions {
        scope: args.scope.into(),
        only: args.only,
        discovery_only: args.discovery_only,
        dry_run: args.dry_run,
        client: ClientOptions {
            delay,
            max_age: args
                .max_age
                .map(|days| Duration::from_secs_f64(days * 86_400.0)),
            conditional: true,
        },
    };
    let state = cache
        .root()
        .parent()
        .map(|parent| parent.join("crawl-state.json"))
        .unwrap_or_else(|| PathBuf::from("crawl-state.json"));
    let transport = UreqTransport::new();
    match crawl::run(&options, &transport, &SystemClock, cache, &state, out) {
        Ok(_) => Ok(ExitCode::SUCCESS),
        Err(error) => {
            writeln!(out, "{error}")?;
            Ok(ExitCode::FAILURE)
        }
    }
}

fn run_index(args: DataDirArgs, cache: &PageCache, out: &mut dyn Write) -> Outcome {
    let merged = crawl::build_index(cache)?;
    let report = &merged.report;
    writeln!(out, "{} player skills indexed", merged.skills.len())?;
    writeln!(
        out,
        "  {} (PvP) titles excluded (D3)",
        report.pvp_excluded.len()
    )?;
    writeln!(
        out,
        "  {} titles on a list page with no id",
        report.titles_without_id.len()
    )?;
    writeln!(
        out,
        "  {} ids on no list page",
        report.ids_without_list_entry.len()
    )?;
    writeln!(
        out,
        "  {} monster or unlisted ids on the game-integration pages",
        report.monster_or_unlisted.len()
    )?;
    writeln!(
        out,
        "  {} unknown campaigns",
        report.unknown_campaigns.len()
    )?;
    writeln!(
        out,
        "  {} id conflicts between the sources",
        report.id_conflicts.len()
    )?;
    // The short lists are printed in full for the findings (T2.3.6 step 4);
    // the monster list is long and only counted.
    for title in &report.titles_without_id {
        writeln!(out, "    no id: {title}")?;
    }
    for skill in &report.ids_without_list_entry {
        writeln!(out, "    on no list: {} {}", skill.id, skill.title)?;
    }
    for (title, campaign) in &report.unknown_campaigns {
        writeln!(out, "    unknown campaign: {title} ({campaign})")?;
    }
    for (id, listed, integration) in &report.id_conflicts {
        writeln!(
            out,
            "    id {id}: skill list {listed:?}, game integration {integration:?}"
        )?;
    }

    let crawled = iso_date(gwsim_extractor::clock::Clock::now_ms(&SystemClock));
    let index = SkillIndexFile {
        provenance: Provenance {
            sources: vec![
                WikiTitle(discovery::SKILL_LIST.to_owned()).url(),
                WikiTitle(discovery::PVE_ONLY_LIST.to_owned()).url(),
                "https://wiki.guildwars.com/wiki/List_of_all_skills".to_owned(),
            ],
            crawled: crawled.parse()?,
            review: ReviewStatus::Draft,
            reviewed_by: None,
            assumptions: Vec::new(),
            notes: "Generated by `gwsim-extract index`; facts only. Regenerate rather than edit."
                .to_owned(),
        },
        skills: merged.skills,
    };
    let path = args.data_dir.join("skills").join("index.ron");
    std::fs::create_dir_all(path.parent().unwrap_or(&args.data_dir))?;
    // The index is regenerated wholesale, not hand-maintained, so unlike
    // seeded files it is replaced (logged as F2.4).
    std::fs::write(&path, seed::write_index(&index))?;
    writeln!(out, "wrote {}", path.display())?;
    Ok(ExitCode::SUCCESS)
}

fn run_seed(args: SeedArgs, cache: &PageCache, out: &mut dyn Write) -> Outcome {
    let scope: Scope = args.scope.into();
    let options = seed::SeedOptions {
        skills: matches!(scope, Scope::Skills | Scope::All),
        foes: matches!(scope, Scope::Foes | Scope::All),
        only: args.only,
        data_dir: args.data_dir,
        dry_run: args.dry_run,
    };
    if options.foes && options.only.is_empty() {
        writeln!(out, "seeding foes needs --only with the foe titles")?;
        return Ok(ExitCode::FAILURE);
    }
    let summary = seed::seed(&options, cache, out)?;
    seed::print_summary(&summary, out)?;
    Ok(if summary.failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn run_diff(args: DiffArgs, cache: &PageCache, out: &mut dyn Write) -> Outcome {
    let scope: Scope = args.scope.into();
    let source = DirSource::new(&args.data_dir);
    let core = CoreData::load(args.data_dir.join("core")).ok();
    let mut updates = Vec::new();
    for title in &args.updates {
        match cache.html(&WikiTitle(title.clone())) {
            Some(body) => updates.extend(gwsim_extractor::update::parse_update(&body, title)),
            None => writeln!(out, "{title} is not cached; crawl it first")?,
        }
    }
    let report = diff::diff(
        &source,
        cache,
        core.as_ref(),
        matches!(scope, Scope::Skills | Scope::All),
        matches!(scope, Scope::Foes | Scope::All),
        &updates,
    )?;
    let markdown = diff::render_markdown(&report);
    match &args.out {
        Some(path) => std::fs::write(path, &markdown)?,
        None => write!(out, "{markdown}")?,
    }
    if let Some(path) = &args.json {
        std::fs::write(
            path,
            serde_json::to_string_pretty(&diff::render_json(&report))?,
        )?;
    }
    Ok(if report.is_clean() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    })
}

fn run_parse_check(
    titles: &[String],
    json: Option<&std::path::Path>,
    cache: &PageCache,
    out: &mut dyn Write,
) -> Outcome {
    let index = crawl::build_index(cache).ok();
    writeln!(
        out,
        "| Skill | Id | Costs | Activation | Recharge | Type | Target / range / AoE | Scaled (r0…r12…r15) |"
    )?;
    writeln!(out, "| --- | --- | --- | --- | --- | --- | --- | --- |")?;
    let mut rows = Vec::new();
    for title in titles {
        let title = WikiTitle(title.clone());
        let index_id = index.as_ref().and_then(|merged| {
            merged
                .skills
                .iter()
                .find(|s| s.title == title)
                .map(|s| s.id)
        });
        let skill = match seed::extract_skill(cache, &title, index_id) {
            Ok(normalised) => normalised.skill,
            Err(reason) => {
                writeln!(out, "| {title} | | could not parse: {reason} | | | | | |")?;
                continue;
            }
        };
        let cost = &skill.cost;
        let mut costs = Vec::new();
        for (label, value) in [
            ("E", cost.energy as i32),
            ("Adr", cost.adrenaline as i32),
            ("Sac%", cost.sacrifice_pct as i32),
            ("Upk", cost.upkeep as i32),
            ("OC", cost.overcast as i32),
        ] {
            if value != 0 {
                costs.push(format!("{label} {value}"));
            }
        }
        let kind = format!(
            "{:?}{}{}",
            skill.kind,
            if skill.elite { ", elite" } else { "" },
            if skill.pve_only { ", PvE-only" } else { "" }
        );
        let targeting = format!(
            "{:?} / {} / {}",
            skill.target,
            skill
                .range
                .map(|r| format!("{r:?}"))
                .unwrap_or_else(|| "—".to_owned()),
            skill
                .aoe
                .map(|a| format!("{a:?}"))
                .unwrap_or_else(|| "—".to_owned())
        );
        let scaled: Vec<String> = skill
            .extracted
            .scaled
            .iter()
            .map(|n| {
                format!(
                    "{}: {}…{}…{}{}",
                    n.label,
                    n.r0,
                    n.r12,
                    n.r15,
                    if n.special_rounding {
                        " (special rounding)"
                    } else {
                        ""
                    }
                )
            })
            .collect();
        writeln!(
            out,
            "| [{}]({}) | {} | {} | {} | {} | {kind} | {targeting} | {} |",
            skill.name,
            title.url(),
            skill.id,
            costs.join(", "),
            skill.activation.as_secs_f64(),
            skill.recharge.as_secs_f64(),
            scaled.join("; ")
        )?;
        rows.push(serde_json::json!({
            "title": title.as_str(),
            "id": skill.id.get(),
            "energy": cost.energy,
            "adrenaline": cost.adrenaline,
            "sacrifice": cost.sacrifice_pct,
            "upkeep": cost.upkeep,
            "overcast": cost.overcast,
            "activation": skill.activation.as_secs_f64(),
            "recharge": skill.recharge.as_secs_f64(),
            "scaled": skill.extracted.scaled.iter().map(|n| [n.r0, n.r12, n.r15]).collect::<Vec<_>>(),
        }));
    }
    if let Some(path) = json {
        std::fs::write(path, serde_json::to_string_pretty(&rows)?)?;
    }
    Ok(ExitCode::SUCCESS)
}

fn run_category_check(category: &str, cache: &PageCache, out: &mut dyn Write) -> Outcome {
    let body = cache
        .html(&WikiTitle(category.to_owned()))
        .ok_or_else(|| format!("{category} is not cached; crawl it with --only first"))?;
    let members = discovery::parse_category_members(&body);
    let merged = crawl::build_index(cache)?;
    let check = discovery::cross_check_category(&members, &merged.skills);
    writeln!(
        out,
        "{category}: {} members on the first page, {} in the index, {} missing",
        members.len(),
        check.matched,
        check.missing_from_index.len()
    )?;
    for title in &check.missing_from_index {
        writeln!(out, "  missing: {title}")?;
    }
    Ok(ExitCode::SUCCESS)
}

fn run_roster(area: &str, cache: &PageCache, out: &mut dyn Write) -> Outcome {
    let body = cache.html(&WikiTitle(area.to_owned())).ok_or_else(|| {
        format!("{area} is not cached; run `gwsim-extract crawl --scope areas --only \"{area}\"`")
    })?;
    let roster = foe::parse_area(&body);
    for (heading, groups) in [("Foes", &roster.groups), ("Bosses", &roster.bosses)] {
        writeln!(out, "{heading}:")?;
        for group in groups.iter() {
            writeln!(out, "  {}", group.name)?;
            for foe in &group.foes {
                let level = foe
                    .level
                    .as_ref()
                    .map(|l| {
                        format!(
                            "{} ({})",
                            l.nm,
                            l.hm.map(|h| h.to_string()).unwrap_or_default()
                        )
                    })
                    .unwrap_or_default();
                let profession = foe.profession.map(|p| format!("{p:?}")).unwrap_or_default();
                writeln!(out, "    {level:<8} {profession:<13} {}", foe.title)?;
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}
