//! The polite HTTP client (T2.2.3, T2.2.4).
//!
//! Every request the extractor makes goes through [`PoliteClient`], which:
//!
//! 1. fetches `robots.txt` first and stops the session if it cannot (EXT-1);
//! 2. refuses any URL that fails the guard or robots.txt, **before** the
//!    transport is called (EXT-1, EXT-2);
//! 3. waits at least the configured delay between request starts, counting
//!    the robots.txt fetch, with one limiter for the whole process (EXT-3);
//! 4. caches every page with its fetch time and sends `If-Modified-Since`
//!    (EXT-5);
//! 5. backs off on 5xx and network errors, and **stops** on 403 or 429
//!    (EXT-6).
//!
//! Crawling is single-threaded: the client takes `&mut self` for every
//! request, so there is no way to have two in flight.

use std::fmt;
use std::time::Duration;

use gwsim_data::WikiTitle;

use crate::cache::{PageCache, PageMeta};
use crate::clock::{Clock, iso_datetime, parse_iso_datetime};
use crate::robots::Robots;
use crate::transport::{Response, Transport, TransportError};
use crate::url::{Refusal, WikiUrl, guard};

/// The default gap between request starts (EXT-3).
pub const DEFAULT_DELAY: Duration = Duration::from_secs(3);
/// The smallest gap a user may ask for (EXT-3).
pub const MIN_DELAY: Duration = Duration::from_secs(2);
/// The waits before each retry of a failing page (EXT-6, T2.2.4).
pub const BACKOFF: [Duration; 3] = [
    Duration::from_secs(30),
    Duration::from_secs(60),
    Duration::from_secs(120),
];
/// How many redirects one fetch may follow.
const MAX_REDIRECTS: usize = 5;

/// The gap between request starts, validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delay(Duration);

impl Delay {
    /// A delay in seconds, refused below [`MIN_DELAY`].
    pub fn from_secs(seconds: f64) -> Result<Delay, String> {
        if !seconds.is_finite() || seconds < MIN_DELAY.as_secs_f64() {
            return Err(format!(
                "--delay {seconds} is too short: the wiki is fetched at most once every {} seconds (EXT-3)",
                MIN_DELAY.as_secs()
            ));
        }
        Ok(Delay(Duration::from_secs_f64(seconds)))
    }

    /// The gap.
    pub fn duration(self) -> Duration {
        self.0
    }
}

impl Default for Delay {
    fn default() -> Self {
        Delay(DEFAULT_DELAY)
    }
}

/// Keeps request starts at least one delay apart.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    gap: Duration,
    last_start: Option<u64>,
}

impl RateLimiter {
    /// A limiter with a gap.
    pub fn new(delay: Delay) -> Self {
        RateLimiter {
            gap: delay.duration(),
            last_start: None,
        }
    }

    /// Waits until the next request may start, then records that it has.
    pub fn wait(&mut self, clock: &dyn Clock) {
        if let Some(last) = self.last_start {
            let ready = last + self.gap.as_millis() as u64;
            let now = clock.now_ms();
            if now < ready {
                clock.sleep(Duration::from_millis(ready - now));
            }
        }
        self.last_start = Some(clock.now_ms());
    }
}

/// How a session is configured.
#[derive(Debug, Clone, Copy)]
pub struct ClientOptions {
    pub delay: Delay,
    /// Pages checked more recently than this are served from the cache
    /// without a request.
    pub max_age: Option<Duration>,
    /// Whether to send `If-Modified-Since` for cached pages. On by default
    /// because T2.1.1 showed the wiki honours it.
    pub conditional: bool,
}

impl Default for ClientOptions {
    fn default() -> Self {
        ClientOptions {
            delay: Delay::default(),
            max_age: None,
            conditional: true,
        }
    }
}

/// What happened to one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchOutcome {
    /// Downloaded and cached. `canonical` is set when the wiki served a
    /// different title, by an HTTP redirect or a MediaWiki redirect page.
    Fetched { canonical: Option<WikiTitle> },
    /// The server said the cached copy is current.
    NotModified,
    /// Checked recently enough that no request was made.
    FromCache,
    /// The page does not exist.
    Missing,
    /// Gave up after the back-off sequence.
    Skipped { reason: String },
    /// Never requested, because the URL is not allowed.
    Refused(Refusal),
}

/// Why a session ended early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// robots.txt could not be read, so nothing may be fetched.
    RobotsUnavailable(String),
    /// The wiki answered 403 or 429.
    Stopped(StopReport),
    /// The cache could not be written.
    Cache(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::RobotsUnavailable(reason) => write!(
                f,
                "robots.txt could not be read ({reason}), so the session has stopped. \
                 Permission is never assumed (EXT-1)."
            ),
            SessionError::Stopped(report) => write!(f, "{report}"),
            SessionError::Cache(reason) => write!(f, "the cache could not be written: {reason}"),
        }
    }
}

impl std::error::Error for SessionError {}

/// The report printed when the wiki refuses us (EXT-6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopReport {
    pub url: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
}

impl fmt::Display for StopReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "STOPPED: {} answered {}. The session has ended immediately.",
            self.url, self.status
        )?;
        writeln!(f, "Headers received:")?;
        for (name, value) in &self.headers {
            writeln!(f, "  {name}: {value}")?;
        }
        write!(
            f,
            "Wait before crawling again — hours, not minutes — and do not retry \
             aggressively. If it happens again, raise the --delay (EXT-6)."
        )
    }
}

/// Counts for the end-of-session summary (T2.2.4 step 3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionSummary {
    pub requests: usize,
    pub fetched: usize,
    pub not_modified: usize,
    pub from_cache: usize,
    pub missing: usize,
    pub skipped: usize,
    pub refused: usize,
    pub stopped: bool,
    pub started_ms: u64,
    pub ended_ms: u64,
}

impl fmt::Display for SessionSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let elapsed = (self.ended_ms.saturating_sub(self.started_ms)) / 1000;
        write!(
            f,
            "{} requests: {} fetched, {} not modified, {} from cache, {} missing, \
             {} skipped, {} refused{}; {}m {}s elapsed",
            self.requests,
            self.fetched,
            self.not_modified,
            self.from_cache,
            self.missing,
            self.skipped,
            self.refused,
            if self.stopped { ", STOPPED" } else { "" },
            elapsed / 60,
            elapsed % 60
        )
    }
}

/// One crawl session's connection to the wiki.
pub struct PoliteClient<'a> {
    transport: &'a dyn Transport,
    clock: &'a dyn Clock,
    cache: &'a PageCache,
    options: ClientOptions,
    limiter: RateLimiter,
    robots: Robots,
    summary: SessionSummary,
}

impl<'a> PoliteClient<'a> {
    /// Opens a session: fetches robots.txt, or stops if it cannot.
    pub fn start(
        transport: &'a dyn Transport,
        clock: &'a dyn Clock,
        cache: &'a PageCache,
        options: ClientOptions,
    ) -> Result<PoliteClient<'a>, SessionError> {
        let mut client = PoliteClient {
            transport,
            clock,
            cache,
            options,
            limiter: RateLimiter::new(options.delay),
            robots: Robots::default(),
            summary: SessionSummary {
                started_ms: clock.now_ms(),
                ended_ms: clock.now_ms(),
                ..SessionSummary::default()
            },
        };

        // The robots.txt fetch counts as a request and waits its turn like
        // any other (T2.2.3 step 2).
        let response = client
            .send(&WikiUrl::robots(), &[])
            .map_err(|error| SessionError::RobotsUnavailable(error.to_string()))?;
        match response.status {
            200 => client.robots = Robots::parse(&response.body),
            403 | 429 => {
                return Err(SessionError::Stopped(StopReport {
                    url: WikiUrl::robots().to_string(),
                    status: response.status,
                    headers: response.headers,
                }));
            }
            other => {
                return Err(SessionError::RobotsUnavailable(format!("status {other}")));
            }
        }
        Ok(client)
    }

    /// Whether robots.txt and the guard allow a URL.
    pub fn check(&self, url: &WikiUrl) -> Result<(), Refusal> {
        guard(url.as_str())?;
        if !self.robots.allows(url.path()) {
            return Err(Refusal::Robots(url.to_string()));
        }
        Ok(())
    }

    /// Fetches one article, or serves it from the cache.
    pub fn fetch(&mut self, title: &WikiTitle) -> Result<FetchOutcome, SessionError> {
        let outcome = self.fetch_inner(title);
        self.summary.ended_ms = self.clock.now_ms();
        match &outcome {
            Ok(FetchOutcome::Fetched { .. }) => self.summary.fetched += 1,
            Ok(FetchOutcome::NotModified) => self.summary.not_modified += 1,
            Ok(FetchOutcome::FromCache) => self.summary.from_cache += 1,
            Ok(FetchOutcome::Missing) => self.summary.missing += 1,
            Ok(FetchOutcome::Skipped { .. }) => self.summary.skipped += 1,
            Ok(FetchOutcome::Refused(_)) => self.summary.refused += 1,
            Err(SessionError::Stopped(_)) => self.summary.stopped = true,
            Err(_) => {}
        }
        outcome
    }

    fn fetch_inner(&mut self, title: &WikiTitle) -> Result<FetchOutcome, SessionError> {
        let cached = self.cache.meta(title);

        if let (Some(meta), Some(max_age)) = (&cached, self.options.max_age)
            && let Some(checked) = parse_iso_datetime(&meta.checked_at)
            && self.clock.now_ms().saturating_sub(checked) < max_age.as_millis() as u64
        {
            return Ok(FetchOutcome::FromCache);
        }

        let mut url = WikiUrl::article(title);
        let mut canonical: Option<WikiTitle> = None;

        let mut headers: Vec<(&str, String)> = Vec::new();
        if self.options.conditional
            && let Some(meta) = &cached
            && self.cache.html(title).is_some()
            && let Some(last_modified) = &meta.last_modified
        {
            headers.push(("If-Modified-Since", last_modified.clone()));
        }

        for _hop in 0..=MAX_REDIRECTS {
            if let Err(refusal) = self.check(&url) {
                return Ok(FetchOutcome::Refused(refusal));
            }
            let response = match self.send_with_backoff(&url, &headers)? {
                Ok(response) => response,
                Err(reason) => return Ok(FetchOutcome::Skipped { reason }),
            };
            let now = iso_datetime(self.clock.now_ms());

            match response.status {
                200 => {
                    // A MediaWiki redirect page is served as a 200 with a
                    // "Redirected from" marker; the canonical title is the
                    // page heading.
                    if canonical.is_none() && crate::html::redirected_from(&response.body).is_some()
                    {
                        canonical = crate::html::page_heading(&response.body).map(WikiTitle);
                    }
                    let meta = PageMeta {
                        url: WikiUrl::article(title).to_string(),
                        title: title.as_str().to_owned(),
                        canonical_title: canonical.as_ref().map(|t| t.as_str().to_owned()),
                        fetched_at: now.clone(),
                        checked_at: now,
                        status: 200,
                        etag: response.header("etag").map(str::to_owned),
                        last_modified: response.header("last-modified").map(str::to_owned),
                    };
                    self.cache
                        .store(&meta, &response.body)
                        .map_err(|error| SessionError::Cache(error.to_string()))?;
                    return Ok(FetchOutcome::Fetched { canonical });
                }
                304 => {
                    self.cache
                        .touch(title, &now)
                        .map_err(|error| SessionError::Cache(error.to_string()))?;
                    return Ok(FetchOutcome::NotModified);
                }
                301 | 302 | 303 | 307 | 308 => {
                    let Some(location) = response.header("location") else {
                        return Ok(FetchOutcome::Skipped {
                            reason: format!("status {} with no Location", response.status),
                        });
                    };
                    match WikiUrl::from_redirect(location) {
                        Ok(target) => {
                            canonical = target.title();
                            url = target;
                            // The conditional header belongs to the old URL.
                            headers.clear();
                        }
                        Err(refusal) => return Ok(FetchOutcome::Refused(refusal)),
                    }
                }
                404 => {
                    let meta = PageMeta {
                        url: WikiUrl::article(title).to_string(),
                        title: title.as_str().to_owned(),
                        canonical_title: None,
                        fetched_at: now.clone(),
                        checked_at: now,
                        status: 404,
                        etag: None,
                        last_modified: None,
                    };
                    self.cache
                        .store_missing(&meta)
                        .map_err(|error| SessionError::Cache(error.to_string()))?;
                    return Ok(FetchOutcome::Missing);
                }
                other => {
                    return Ok(FetchOutcome::Skipped {
                        reason: format!("unexpected status {other}"),
                    });
                }
            }
        }

        Ok(FetchOutcome::Skipped {
            reason: format!("more than {MAX_REDIRECTS} redirects"),
        })
    }

    /// Sends a request, retrying 5xx and network errors on the back-off
    /// schedule and stopping the session on 403 or 429.
    ///
    /// The outer `Result` is the session ending; the inner one is this page
    /// being given up on.
    fn send_with_backoff(
        &mut self,
        url: &WikiUrl,
        headers: &[(&str, String)],
    ) -> Result<Result<Response, String>, SessionError> {
        let mut attempt = 0usize;
        loop {
            let result = self.send(url, headers);
            let retryable = match &result {
                Ok(response) if response.status == 403 || response.status == 429 => {
                    return Err(SessionError::Stopped(StopReport {
                        url: url.to_string(),
                        status: response.status,
                        headers: response.headers.clone(),
                    }));
                }
                Ok(response) if response.status >= 500 => format!("status {}", response.status),
                Ok(response) => return Ok(Ok(response.clone())),
                Err(error) => error.to_string(),
            };
            match BACKOFF.get(attempt) {
                Some(wait) => {
                    self.clock.sleep(*wait);
                    attempt += 1;
                }
                None => {
                    return Ok(Err(format!("{retryable} after {} retries", BACKOFF.len())));
                }
            }
        }
    }

    /// One request, after the limiter.
    fn send(
        &mut self,
        url: &WikiUrl,
        headers: &[(&str, String)],
    ) -> Result<Response, TransportError> {
        // The guard runs here too, so even robots.txt passes it; this is the
        // last line before the network.
        if let Err(refusal) = guard(url.as_str()) {
            return Err(TransportError(refusal.to_string()));
        }
        self.limiter.wait(self.clock);
        self.summary.requests += 1;
        self.transport.get(url, headers)
    }

    /// The summary so far.
    pub fn summary(&self) -> &SessionSummary {
        &self.summary
    }

    /// Ends the session, returning its summary.
    pub fn finish(mut self) -> SessionSummary {
        self.summary.ended_ms = self.clock.now_ms();
        self.summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_below_two_seconds_are_refused() {
        assert!(Delay::from_secs(2.0).is_ok());
        assert!(Delay::from_secs(3.5).is_ok());
        assert!(Delay::from_secs(1.0).is_err());
        assert!(Delay::from_secs(1.999).is_err());
        assert!(Delay::from_secs(f64::NAN).is_err());
        assert_eq!(Delay::default().duration(), Duration::from_secs(3));
    }

    #[test]
    fn the_limiter_waits_only_for_what_is_left_of_the_gap() {
        let clock = crate::clock::FakeClock::at(0);
        let mut limiter = RateLimiter::new(Delay::default());
        limiter.wait(&clock);
        clock.advance(Duration::from_millis(1200));
        limiter.wait(&clock);
        clock.advance(Duration::from_secs(10));
        limiter.wait(&clock);
        assert_eq!(clock.sleeps(), vec![Duration::from_millis(1800)]);
    }
}
