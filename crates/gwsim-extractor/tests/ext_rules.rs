//! One test per crawl rule, EXT-1 to EXT-7 (T2.2.6).
//!
//! Every test runs against [`FakeTransport`] and [`FakeClock`]: no test here
//! touches the network, and none really sleeps.

mod common;

use std::time::Duration;

use common::{TempDir, fixture};
use gwsim_data::WikiTitle;
use gwsim_extractor::cache::PageCache;
use gwsim_extractor::client::{
    BACKOFF, ClientOptions, Delay, FetchOutcome, PoliteClient, SessionError,
};
use gwsim_extractor::clock::FakeClock;
use gwsim_extractor::transport::{FakeTransport, Response};
use gwsim_extractor::url::{ROBOTS_URL, Refusal};

const ES: &str = "https://wiki.guildwars.com/wiki/Energy_Surge";

fn title(text: &str) -> WikiTitle {
    WikiTitle(text.to_owned())
}

fn page(body: &str) -> Response {
    Response::new(200, body).with_header("Last-Modified", "Tue, 22 Sep 2026 20:07:31 GMT")
}

// ------------------------------------------------------------------ EXT-1

#[test]
fn ext1_robots_is_fetched_first() {
    let dir = TempDir::new("ext1-first");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport.respond(ES, page("x"));
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    client.fetch(&title("Energy Surge")).unwrap();

    assert_eq!(transport.urls(), vec![ROBOTS_URL, ES]);
}

#[test]
fn ext1_robots_disallow_is_respected() {
    let dir = TempDir::new("ext1-disallow");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new();
    transport.respond(
        ROBOTS_URL,
        Response::new(200, "User-agent: *\nDisallow: /wiki/Energy\n"),
    );
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    let outcome = client.fetch(&title("Energy Surge")).unwrap();

    assert!(matches!(outcome, FetchOutcome::Refused(Refusal::Robots(_))));
    assert_eq!(
        transport.urls(),
        vec![ROBOTS_URL],
        "nothing after robots.txt"
    );
}

#[test]
fn ext1_api_php_never_requested() {
    let dir = TempDir::new("ext1-api");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    // A redirect towards a forbidden endpoint must not be followed.
    transport.respond(
        ES,
        Response::new(301, "").with_header("Location", "/api.php?action=raw&title=Energy_Surge"),
    );
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    let redirected = client.fetch(&title("Energy Surge")).unwrap();
    let special = client.fetch(&title("Special:Random")).unwrap();

    assert!(matches!(redirected, FetchOutcome::Refused(_)));
    assert!(matches!(
        special,
        FetchOutcome::Refused(Refusal::SpecialPage(_))
    ));
    for url in transport.urls() {
        assert!(!url.contains("api.php"), "{url}");
        assert!(!url.to_lowercase().contains("special:"), "{url}");
        assert!(!url.contains("index.php"), "{url}");
    }
}

#[test]
fn ext1_an_unreadable_robots_txt_stops_the_session() {
    let dir = TempDir::new("ext1-unreadable");
    let cache = PageCache::new(dir.path());
    let clock = FakeClock::default();

    for transport in [
        {
            let t = FakeTransport::new();
            t.respond(ROBOTS_URL, Response::new(404, ""));
            t
        },
        {
            let t = FakeTransport::new();
            t.fail(ROBOTS_URL, "connection refused");
            t
        },
    ] {
        let result = PoliteClient::start(&transport, &clock, &cache, ClientOptions::default());
        assert!(matches!(result, Err(SessionError::RobotsUnavailable(_))));
        assert_eq!(transport.urls(), vec![ROBOTS_URL]);
    }
}

// ------------------------------------------------------------------ EXT-2

#[test]
fn ext2_only_article_urls() {
    let dir = TempDir::new("ext2");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport.respond(
        "https://wiki.guildwars.com/wiki/Guild_Wars_Wiki:Game_integration/Skills/1-500",
        page("x"),
    );
    transport.respond(
        "https://wiki.guildwars.com/wiki/Category:Mesmer_skills",
        page("x"),
    );
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    for name in [
        "Guild Wars Wiki:Game integration/Skills/1-500",
        "Category:Mesmer skills",
        "Why?",
        "Energy Surge",
    ] {
        client.fetch(&title(name)).unwrap();
    }

    for url in transport.urls() {
        assert!(
            url == ROBOTS_URL || url.starts_with("https://wiki.guildwars.com/wiki/"),
            "{url} is neither robots.txt nor an article"
        );
        assert!(url == ROBOTS_URL || !url.contains('?'), "{url}");
    }
}

// ------------------------------------------------------------------ EXT-3

#[test]
fn ext3_minimum_delay() {
    let dir = TempDir::new("ext3");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    for name in ["A", "B", "C"] {
        client.fetch(&title(name)).unwrap();
    }

    // robots.txt counts as a request, so every page waits a full gap.
    assert_eq!(clock.sleeps(), vec![Duration::from_secs(3); 3]);
    assert_eq!(client.summary().requests, 4);
}

#[test]
fn ext3_delay_of_two_is_accepted_and_one_is_rejected() {
    assert!(Delay::from_secs(2.0).is_ok());
    let error = Delay::from_secs(1.0).unwrap_err();
    assert!(error.contains("EXT-3"), "{error}");
}

#[test]
fn ext3_a_custom_delay_is_honoured() {
    let dir = TempDir::new("ext3-custom");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    let clock = FakeClock::default();
    let options = ClientOptions {
        delay: Delay::from_secs(5.0).unwrap(),
        ..ClientOptions::default()
    };

    let mut client = PoliteClient::start(&transport, &clock, &cache, options).unwrap();
    client.fetch(&title("A")).unwrap();
    assert_eq!(clock.sleeps(), vec![Duration::from_secs(5)]);
}

// ------------------------------------------------------------------ EXT-4

#[test]
fn ext4_user_agent_has_no_personal_details() {
    let agent = gwsim_extractor::user_agent();
    assert!(agent.starts_with("gwsim-extractor/"), "{agent}");
    assert!(agent.contains("(+https://"), "{agent}");
    assert!(!agent.contains('@'), "{agent}");
}

// ------------------------------------------------------------------ EXT-5

#[test]
fn ext5_every_page_is_cached_with_its_fetch_time() {
    let dir = TempDir::new("ext5-cached");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport.respond(ES, page("<html>body</html>"));
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    client.fetch(&title("Energy Surge")).unwrap();

    let meta = cache
        .meta(&title("Energy Surge"))
        .expect("meta should be written");
    assert_eq!(meta.status, 200);
    assert_eq!(meta.fetched_at, "2026-09-23T00:00:03Z");
    assert_eq!(
        meta.last_modified.as_deref(),
        Some("Tue, 22 Sep 2026 20:07:31 GMT")
    );
    assert_eq!(
        cache.html(&title("Energy Surge")).as_deref(),
        Some("<html>body</html>")
    );
}

#[test]
fn ext5_cache_hit_avoids_request() {
    let dir = TempDir::new("ext5-hit");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport.respond(ES, page("x"));
    let clock = FakeClock::default();
    let options = ClientOptions {
        max_age: Some(Duration::from_secs(86_400)),
        ..ClientOptions::default()
    };

    let mut client = PoliteClient::start(&transport, &clock, &cache, options).unwrap();
    client.fetch(&title("Energy Surge")).unwrap();
    clock.advance(Duration::from_secs(3600));
    let second = client.fetch(&title("Energy Surge")).unwrap();

    assert_eq!(second, FetchOutcome::FromCache);
    assert_eq!(
        transport.urls(),
        vec![ROBOTS_URL, ES],
        "the second fetch made no request"
    );
}

#[test]
fn ext5_a_conditional_request_keeps_the_cached_copy_on_304() {
    let dir = TempDir::new("ext5-304");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport
        .respond(ES, page("original"))
        .respond(ES, Response::new(304, ""));
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    client.fetch(&title("Energy Surge")).unwrap();
    let second = client.fetch(&title("Energy Surge")).unwrap();

    assert_eq!(second, FetchOutcome::NotModified);
    let request = &transport.requests()[2];
    assert_eq!(
        request.headers,
        vec![(
            "If-Modified-Since".to_owned(),
            "Tue, 22 Sep 2026 20:07:31 GMT".to_owned()
        )]
    );
    assert_eq!(
        cache.html(&title("Energy Surge")).as_deref(),
        Some("original")
    );
    let meta = cache.meta(&title("Energy Surge")).unwrap();
    assert_ne!(
        meta.checked_at, meta.fetched_at,
        "the check time moves on a 304"
    );
}

// ------------------------------------------------------------------ EXT-6

#[test]
fn ext6_429_stops_session() {
    let dir = TempDir::new("ext6-429");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport.respond(
        ES,
        Response::new(429, "").with_header("Retry-After", "3600"),
    );
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    let result = client.fetch(&title("Energy Surge"));

    let Err(SessionError::Stopped(report)) = result else {
        panic!("a 429 must stop the session, got {result:?}");
    };
    assert_eq!(report.status, 429);
    let text = report.to_string();
    assert!(text.contains("retry-after: 3600"), "{text}");
    assert!(text.contains("do not retry"), "{text}");
    assert!(client.summary().stopped);
    // No retry: exactly one request to the page.
    assert_eq!(transport.urls(), vec![ROBOTS_URL, ES]);
}

#[test]
fn ext6_403_stops_session() {
    let dir = TempDir::new("ext6-403");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport.respond(ES, Response::new(403, ""));
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    assert!(matches!(
        client.fetch(&title("Energy Surge")),
        Err(SessionError::Stopped(_))
    ));
}

#[test]
fn ext6_5xx_backoff_sequence() {
    let dir = TempDir::new("ext6-5xx");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    for _ in 0..4 {
        transport.respond(ES, Response::new(503, ""));
    }
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    let outcome = client.fetch(&title("Energy Surge")).unwrap();

    assert!(
        matches!(outcome, FetchOutcome::Skipped { .. }),
        "{outcome:?}"
    );
    // The first attempt waits the ordinary gap; each retry waits its back-off
    // step and is then already past the gap, so it waits nothing more.
    let expected: Vec<Duration> = std::iter::once(Duration::from_secs(3))
        .chain(BACKOFF)
        .collect();
    assert_eq!(clock.sleeps(), expected);
    assert_eq!(
        transport.urls().len(),
        1 + 4,
        "robots plus one try and three retries"
    );
}

#[test]
fn ext6_a_network_error_is_treated_like_a_5xx_and_can_recover() {
    let dir = TempDir::new("ext6-net");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport.fail(ES, "reset").respond(ES, page("ok"));
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    let outcome = client.fetch(&title("Energy Surge")).unwrap();

    assert!(matches!(outcome, FetchOutcome::Fetched { .. }));
    assert_eq!(clock.sleeps()[1], BACKOFF[0]);
}

// -------------------------------------------------------- other statuses

#[test]
fn a_404_is_recorded_and_the_crawl_continues() {
    let dir = TempDir::new("status-404");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport.respond(ES, page("x"));
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    assert_eq!(
        client.fetch(&title("Nowhere")).unwrap(),
        FetchOutcome::Missing
    );
    assert!(matches!(
        client.fetch(&title("Energy Surge")).unwrap(),
        FetchOutcome::Fetched { .. }
    ));
    assert_eq!(cache.meta(&title("Nowhere")).unwrap().status, 404);
    assert_eq!(client.summary().missing, 1);
}

#[test]
fn an_allowed_http_redirect_is_followed_and_the_canonical_title_recorded() {
    let dir = TempDir::new("status-301");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport.respond(
        "https://wiki.guildwars.com/wiki/ES",
        Response::new(301, "").with_header("Location", "/wiki/Energy_Surge"),
    );
    transport.respond(ES, page("canonical"));
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    let outcome = client.fetch(&title("ES")).unwrap();

    assert_eq!(
        outcome,
        FetchOutcome::Fetched {
            canonical: Some(title("Energy Surge"))
        }
    );
    let meta = cache.meta(&title("ES")).unwrap();
    assert_eq!(meta.canonical_title.as_deref(), Some("Energy Surge"));
    assert_eq!(cache.html(&title("ES")).as_deref(), Some("canonical"));
}

#[test]
fn a_mediawiki_redirect_page_records_its_canonical_title() {
    let dir = TempDir::new("status-redirect-page");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport.respond(
        "https://wiki.guildwars.com/wiki/Lorem_Surge_old_name",
        page(&fixture("skill_redirected.html")),
    );
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    let outcome = client.fetch(&title("Lorem Surge old name")).unwrap();

    assert_eq!(
        outcome,
        FetchOutcome::Fetched {
            canonical: Some(title("Lorem Surge"))
        }
    );
}

#[test]
fn the_session_summary_counts_every_outcome() {
    let dir = TempDir::new("summary");
    let cache = PageCache::new(dir.path());
    let transport = FakeTransport::new().with_wiki_robots();
    transport.respond(ES, page("x"));
    let clock = FakeClock::default();

    let mut client =
        PoliteClient::start(&transport, &clock, &cache, ClientOptions::default()).unwrap();
    client.fetch(&title("Energy Surge")).unwrap();
    client.fetch(&title("Nowhere")).unwrap();
    client.fetch(&title("Special:Random")).unwrap();
    let summary = client.finish();

    assert_eq!(summary.fetched, 1);
    assert_eq!(summary.missing, 1);
    assert_eq!(summary.refused, 1);
    let text = summary.to_string();
    assert!(text.contains("1 fetched"), "{text}");
    assert!(text.contains("elapsed"), "{text}");
}
