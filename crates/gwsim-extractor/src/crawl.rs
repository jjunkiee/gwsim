//! `gwsim-extract crawl`: refresh the cache, resumably (T2.3.4).
//!
//! The order of work is fixed:
//!
//! 1. the discovery pages (skill lists, area pages);
//! 2. the page list, worked out from what discovery cached;
//! 3. each page, one at a time, through the polite client;
//! 4. the crawl state, saved after every page, so an interrupted crawl
//!    resumes where it stopped.
//!
//! `--only` skips discovery and fetches exactly the titles given.

use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use gwsim_data::WikiTitle;

use crate::cache::{CrawlState, PageCache};
use crate::client::{ClientOptions, FetchOutcome, PoliteClient, SessionError, SessionSummary};
use crate::clock::Clock;
use crate::discovery;
use crate::foe::parse_area;
use crate::transport::Transport;
use crate::url::WikiUrl;

/// The areas M1 needs, used when `--scope areas` or `foes` is given without
/// `--only`.
pub const M1_AREAS: [&str; 1] = ["Vehtendi Valley"];

/// What to crawl.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Skills,
    Foes,
    Areas,
    All,
}

impl Scope {
    fn includes(self, other: Scope) -> bool {
        self == Scope::All || self == other
    }
}

/// How a crawl was asked for.
#[derive(Debug, Clone)]
pub struct CrawlOptions {
    pub scope: Scope,
    /// Exact titles to fetch. Skips discovery.
    pub only: Vec<String>,
    /// Fetch the discovery pages and stop.
    pub discovery_only: bool,
    /// Print the URLs and fetch nothing.
    pub dry_run: bool,
    pub client: ClientOptions,
}

/// What a crawl did.
#[derive(Debug, Clone, Default)]
pub struct CrawlReport {
    pub outcomes: Vec<(WikiTitle, FetchOutcome)>,
    pub summary: SessionSummary,
}

/// The discovery pages a scope needs.
pub fn discovery_pages(scope: Scope) -> Vec<WikiTitle> {
    let mut pages = Vec::new();
    if scope.includes(Scope::Skills) {
        pages.extend(discovery::discovery_titles());
    }
    if scope.includes(Scope::Areas) || scope.includes(Scope::Foes) {
        pages.extend(M1_AREAS.iter().map(|a| WikiTitle((*a).to_owned())));
    }
    pages
}

/// The pages discovery points at, read from what is cached.
pub fn discovered_pages(scope: Scope, cache: &PageCache) -> Vec<WikiTitle> {
    let mut pages = Vec::new();
    if scope.includes(Scope::Skills)
        && let Ok(merged) = build_index(cache)
    {
        pages.extend(merged.skills.into_iter().map(|skill| skill.title));
    }
    if scope.includes(Scope::Foes) {
        for area in M1_AREAS {
            if let Some(body) = cache.html(&WikiTitle(area.to_owned())) {
                pages.extend(parse_area(&body).foe_titles().into_iter().map(WikiTitle));
            }
        }
    }
    pages.sort();
    pages.dedup();
    pages
}

/// Rebuilds the merged skill index from the cached discovery pages.
pub fn build_index(cache: &PageCache) -> Result<discovery::Merged, String> {
    let read = |title: &str| {
        cache
            .html(&WikiTitle(title.to_owned()))
            .ok_or_else(|| format!("{title:?} is not cached; run `gwsim-extract crawl --scope skills --discovery-only` first"))
    };

    let listed = discovery::parse_skill_list(&read(discovery::SKILL_LIST)?);
    let pve_only = discovery::parse_named_rows(&read(discovery::PVE_ONLY_LIST)?);
    let mut professions = Vec::new();
    for profession in gwsim_data::core::Profession::ALL
        .map(Some)
        .into_iter()
        .chain([None])
    {
        let title = discovery::profession_list_title(profession);
        professions.extend(discovery::parse_profession_list(
            &read(title.as_str())?,
            profession,
        ));
    }
    let mut game_integration = Vec::new();
    for range in discovery::GAME_INTEGRATION_RANGES {
        // Optional: the cross-check still works with whatever is cached.
        if let Some(body) = cache.html(&discovery::game_integration_title(range)) {
            game_integration.extend(discovery::parse_game_integration(&body).rows);
        }
    }
    Ok(discovery::merge(
        &listed,
        &professions,
        &pve_only,
        &game_integration,
    ))
}

/// A duration as "about 3.3 hours" or "about 4 minutes".
pub fn eta(pages: usize, delay: Duration) -> String {
    let seconds = pages as f64 * delay.as_secs_f64();
    if seconds >= 3600.0 {
        format!("about {:.1} hours", seconds / 3600.0)
    } else if seconds >= 90.0 {
        format!("about {:.0} minutes", seconds / 60.0)
    } else if seconds >= 60.0 {
        "about 1 minute".to_owned()
    } else {
        format!("about {seconds:.0} seconds")
    }
}

/// Runs a crawl.
pub fn run(
    options: &CrawlOptions,
    transport: &dyn Transport,
    clock: &dyn Clock,
    cache: &PageCache,
    state_path: &Path,
    out: &mut dyn Write,
) -> Result<CrawlReport, CrawlError> {
    let delay = options.client.delay.duration();
    let first: Vec<WikiTitle> = if options.only.is_empty() {
        discovery_pages(options.scope)
    } else {
        options.only.iter().map(|t| WikiTitle(t.clone())).collect()
    };

    let mut state = CrawlState::load(state_path);
    let resumed = !state.completed.is_empty();

    if options.dry_run {
        writeln!(
            out,
            "dry run: {} pages, {}",
            first.len(),
            eta(first.len(), delay)
        )?;
        for title in &first {
            writeln!(out, "  {}", WikiUrl::article(title))?;
        }
        if options.only.is_empty() && !options.discovery_only {
            let later = discovered_pages(options.scope, cache);
            writeln!(
                out,
                "then {} pages found by discovery (from the current cache)",
                later.len()
            )?;
            for title in &later {
                writeln!(out, "  {}", WikiUrl::article(title))?;
            }
        }
        return Ok(CrawlReport::default());
    }

    let mut client = PoliteClient::start(transport, clock, cache, options.client)?;
    let mut report = CrawlReport::default();
    if resumed {
        writeln!(
            out,
            "resuming: {} pages already done",
            state.completed.len()
        )?;
    }

    state.enqueue(first.iter().map(|t| t.0.clone()));
    writeln!(
        out,
        "{} pages to fetch, {}",
        state.pending.len(),
        eta(state.pending.len(), delay)
    )?;
    work_through(&mut client, &mut state, state_path, &mut report, out)?;

    if options.only.is_empty() && !options.discovery_only {
        let later = discovered_pages(options.scope, cache);
        state.enqueue(later.into_iter().map(|t| t.0));
        writeln!(
            out,
            "discovery found {} more pages, {}",
            state.pending.len(),
            eta(state.pending.len(), delay)
        )?;
        work_through(&mut client, &mut state, state_path, &mut report, out)?;
    }

    // Finished cleanly: the next crawl starts afresh.
    let _ = std::fs::remove_file(state_path);
    report.summary = client.finish();
    writeln!(out, "{}", report.summary)?;
    Ok(report)
}

fn work_through(
    client: &mut PoliteClient<'_>,
    state: &mut CrawlState,
    state_path: &Path,
    report: &mut CrawlReport,
    out: &mut dyn Write,
) -> Result<(), CrawlError> {
    while let Some(title) = state.pending.first().cloned() {
        let outcome = match client.fetch(&WikiTitle(title.clone())) {
            Ok(outcome) => outcome,
            Err(error) => {
                // Save what is done so a later crawl resumes here.
                state.save(state_path)?;
                writeln!(out, "{error}")?;
                writeln!(out, "{}", client.summary())?;
                return Err(CrawlError::Session(error));
            }
        };
        writeln!(out, "  {title}: {}", describe(&outcome))?;
        report.outcomes.push((WikiTitle(title.clone()), outcome));
        state.complete(&title);
        state.save(state_path)?;
    }
    Ok(())
}

fn describe(outcome: &FetchOutcome) -> String {
    match outcome {
        FetchOutcome::Fetched { canonical: None } => "fetched".to_owned(),
        FetchOutcome::Fetched {
            canonical: Some(title),
        } => format!("fetched (redirects to {title})"),
        FetchOutcome::NotModified => "not modified".to_owned(),
        FetchOutcome::FromCache => "cached".to_owned(),
        FetchOutcome::Missing => "missing (404)".to_owned(),
        FetchOutcome::Skipped { reason } => format!("skipped: {reason}"),
        FetchOutcome::Refused(refusal) => format!("refused: {refusal}"),
    }
}

/// Why a crawl ended early.
#[derive(Debug)]
pub enum CrawlError {
    Session(SessionError),
    Io(io::Error),
}

impl std::fmt::Display for CrawlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CrawlError::Session(error) => write!(f, "{error}"),
            CrawlError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CrawlError {}

impl From<SessionError> for CrawlError {
    fn from(error: SessionError) -> Self {
        CrawlError::Session(error)
    }
}

impl From<io::Error> for CrawlError {
    fn from(error: io::Error) -> Self {
        CrawlError::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_eta_reads_in_sensible_units() {
        assert_eq!(eta(4000, Duration::from_secs(3)), "about 3.3 hours");
        assert_eq!(eta(72, Duration::from_secs(3)), "about 4 minutes");
        assert_eq!(eta(9, Duration::from_secs(3)), "about 27 seconds");
    }
}
