//! Seeding and diffing over the fixture cache (T2.6.4, T2.7.5), and the
//! crawl's resumability (T2.3.4).

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{TempDir, fixture};
use gwsim_data::core::CoreData;
use gwsim_data::dataset::DataSet;
use gwsim_data::{DirSource, WikiTitle};
use gwsim_extractor::cache::{PageCache, PageMeta};
use gwsim_extractor::client::ClientOptions;
use gwsim_extractor::clock::FakeClock;
use gwsim_extractor::crawl::{self, CrawlOptions, Scope};
use gwsim_extractor::diff::{self, Change};
use gwsim_extractor::seed::{self, SeedOptions};
use gwsim_extractor::transport::{FakeTransport, Response};

fn repo_data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A data directory holding the repository's core, item and assumption
/// files and nothing else, ready to seed into.
fn scratch_data(root: &Path) -> PathBuf {
    let data = root.join("data");
    copy_dir(&repo_data().join("core"), &data.join("core"));
    copy_dir(&repo_data().join("items"), &data.join("items"));
    fs::copy(
        repo_data().join("assumptions.ron"),
        data.join("assumptions.ron"),
    )
    .unwrap();
    data
}

fn store(cache: &PageCache, title: &str, body: &str) {
    cache
        .store(
            &PageMeta {
                url: WikiTitle(title.to_owned()).url(),
                title: title.to_owned(),
                canonical_title: None,
                fetched_at: "2026-09-23T00:00:00Z".to_owned(),
                checked_at: "2026-09-23T00:00:00Z".to_owned(),
                status: 200,
                etag: None,
                last_modified: None,
            },
            body,
        )
        .unwrap();
}

/// Skill pages the fixture foes name, made from the basic fixture with a
/// new title and id so every foe reference resolves.
const EXTRA_SKILLS: [(&str, u16); 6] = [
    ("Drain Enchantment", 9101),
    ("Power Spike", 9102),
    ("Executioner's Strike", 9103),
    ("Magehunter Strike", 9104),
    ("Sprint", 9105),
    ("Mighty Blow", 9106),
];

const SKILL_FIXTURES: [(&str, &str); 6] = [
    ("skill_basic.html", "Lorem Surge"),
    ("skill_split_pve.html", "Ipsum Hex"),
    ("skill_title.html", "Dolor of Superiority"),
    ("skill_adrenaline.html", "Sit Chop"),
    ("skill_sacrifice.html", "Amet is Power"),
    ("skill_special_rounding.html", "Consectetur Spike"),
];

/// A cache holding every fixture page under its title.
fn fixture_cache(root: &Path) -> PageCache {
    let cache = PageCache::new(root.join("cache"));
    for (file, title) in SKILL_FIXTURES {
        store(&cache, title, &fixture(file));
    }
    store(
        &cache,
        "Adipiscing Laughter",
        &fixture("skill_monster.html"),
    );
    for (title, id) in EXTRA_SKILLS {
        let body = fixture("skill_basic.html")
            .replace("Lorem Surge", title)
            .replace("9001", &id.to_string());
        store(&cache, title, &body);
    }
    store(&cache, "Lorem Seer", &fixture("foe_basic.html"));
    store(&cache, "Ipsum Guard", &fixture("foe_variants.html"));
    cache
}

fn all_skill_titles() -> Vec<String> {
    SKILL_FIXTURES
        .iter()
        .map(|(_, t)| t.to_string())
        .chain(["Adipiscing Laughter".to_owned()])
        .chain(EXTRA_SKILLS.iter().map(|(t, _)| t.to_string()))
        .collect()
}

fn seed_everything(cache: &PageCache, data: &Path) -> seed::SeedSummary {
    let mut sink = Vec::new();
    let skills = seed::seed(
        &SeedOptions {
            skills: true,
            foes: false,
            only: all_skill_titles(),
            data_dir: data.to_path_buf(),
            dry_run: false,
        },
        cache,
        &mut sink,
    )
    .unwrap();
    let foes = seed::seed(
        &SeedOptions {
            skills: false,
            foes: true,
            only: vec!["Lorem Seer".to_owned(), "Ipsum Guard".to_owned()],
            data_dir: data.to_path_buf(),
            dry_run: false,
        },
        cache,
        &mut sink,
    )
    .unwrap();
    seed::SeedSummary {
        created: [skills.created, foes.created].concat(),
        existed: [skills.existed, foes.existed].concat(),
        failed: [skills.failed, foes.failed].concat(),
        warnings: [skills.warnings, foes.warnings].concat(),
    }
}

fn written_files(data: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for folder in ["skills", "creatures"] {
        let mut stack = vec![data.join(folder)];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                if entry.file_type().unwrap().is_dir() {
                    stack.push(entry.path());
                } else {
                    files.push(entry.path());
                }
            }
        }
    }
    files.sort();
    files
}

// -------------------------------------------------------------------- seed

#[test]
fn seeded_files_validate_with_the_core_data() {
    let dir = TempDir::new("seed-validate");
    let data = scratch_data(dir.path());
    let cache = fixture_cache(dir.path());

    let summary = seed_everything(&cache, &data);
    assert!(summary.failed.is_empty(), "{:?}", summary.failed);
    assert_eq!(summary.created.len(), 13 + 2);

    let loaded = DataSet::load(&DirSource::new(&data));
    let loaded =
        loaded.unwrap_or_else(|problems| panic!("seeded data should validate:\n{problems}"));
    assert_eq!(loaded.skills.len(), 13);
    assert_eq!(loaded.foes.len(), 2);

    // Folders follow the profession rules (§8.1).
    assert!(data.join("skills/mesmer/lorem-surge.ron").exists());
    assert!(data.join("skills/common/dolor-of-superiority.ron").exists());
    assert!(data.join("skills/monster/adipiscing-laughter.ron").exists());
    assert!(
        data.join("creatures/foes/lorem-military/lorem-seer.ron")
            .exists()
    );
}

#[test]
fn a_seeded_foe_is_draft_with_its_assumptions() {
    let dir = TempDir::new("seed-foe");
    let data = scratch_data(dir.path());
    let cache = fixture_cache(dir.path());
    seed_everything(&cache, &data);

    let loaded = DataSet::load(&DirSource::new(&data)).unwrap();
    let guard = loaded.foe(&"ipsum-guard".parse().unwrap()).unwrap();
    assert_eq!(guard.provenance.review, gwsim_data::ReviewStatus::Draft);
    let ids: Vec<String> = guard
        .provenance
        .assumptions
        .iter()
        .map(|a| a.to_string())
        .collect();
    // No weapon (A-004), no hard-mode ranks (A-005), variants (A-007).
    assert_eq!(ids, ["A-004", "A-005", "A-007"]);
    assert_eq!((guard.level.nm, guard.level.hm), (19, Some(27)));
    assert_eq!(guard.variants.len(), 1);
    assert_eq!(guard.armor.default, Some(91));
    assert_eq!(guard.armor.per_type.len(), 3, "the physical types differ");
    assert_eq!(guard.armor.level_context, Some(19));

    let seer = loaded.foe(&"lorem-seer".parse().unwrap()).unwrap();
    let ids: Vec<String> = seer
        .provenance
        .assumptions
        .iter()
        .map(|a| a.to_string())
        .collect();
    assert_eq!(
        ids,
        ["A-004"],
        "the page gives hard-mode ranks, so no A-005"
    );
    assert!(seer.skills[3].hm_only);
}

#[test]
fn a_second_seed_creates_nothing_and_changes_nothing() {
    let dir = TempDir::new("seed-twice");
    let data = scratch_data(dir.path());
    let cache = fixture_cache(dir.path());
    seed_everything(&cache, &data);

    let before: Vec<(PathBuf, Vec<u8>)> = written_files(&data)
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();

    let second = seed_everything(&cache, &data);
    assert!(second.created.is_empty(), "{:?}", second.created);
    assert_eq!(second.existed.len(), before.len());
    for (path, bytes) in before {
        assert_eq!(
            fs::read(&path).unwrap(),
            bytes,
            "{} changed",
            path.display()
        );
    }
}

#[test]
fn an_existing_hand_edited_file_is_byte_identical_after_seed() {
    let dir = TempDir::new("seed-existing");
    let data = scratch_data(dir.path());
    let cache = fixture_cache(dir.path());
    let path = data.join("skills/mesmer/lorem-surge.ron");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "// hand edited, not valid on purpose\n").unwrap();

    let summary = seed_everything(&cache, &data);
    assert!(
        summary
            .existed
            .contains(&PathBuf::from("skills/mesmer/lorem-surge.ron"))
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "// hand edited, not valid on purpose\n"
    );
}

#[test]
fn a_dry_run_writes_nothing() {
    let dir = TempDir::new("seed-dry");
    let data = scratch_data(dir.path());
    let cache = fixture_cache(dir.path());
    let mut output = Vec::new();
    let summary = seed::seed(
        &SeedOptions {
            skills: true,
            foes: false,
            only: all_skill_titles(),
            data_dir: data.clone(),
            dry_run: true,
        },
        &cache,
        &mut output,
    )
    .unwrap();
    assert_eq!(summary.created.len(), 13);
    assert!(!data.join("skills").exists());
    assert!(String::from_utf8(output).unwrap().contains("would create"));
}

#[test]
fn ext7_seed_output_contains_no_description_text() {
    let dir = TempDir::new("seed-ext7");
    let data = scratch_data(dir.path());
    let cache = fixture_cache(dir.path());
    seed_everything(&cache, &data);

    // Every piece of placeholder prose the fixtures carry.
    let prose = [
        "Lorem skill text",
        "Ipsum",
        "dolor.",
        "Lorem concise text",
        "Lorem note",
        "lorem ipsum",
        "unusable with sword",
        "level 20 and above only",
    ];
    for path in written_files(&data) {
        let text = fs::read_to_string(&path).unwrap();
        for phrase in prose {
            // "Ipsum" is also a title word ("Ipsum Hex", "Ipsum Guard"), which
            // is a fact, so only prose-shaped uses count.
            if phrase == "Ipsum" {
                assert!(!text.contains("Ipsum <span"), "{}", path.display());
                continue;
            }
            assert!(
                !text.contains(phrase),
                "{} contains {phrase:?}",
                path.display()
            );
        }
    }
}

#[test]
fn seeded_skill_files_round_trip_through_the_writer() {
    let dir = TempDir::new("seed-roundtrip");
    let cache = fixture_cache(dir.path());
    for (_, title) in SKILL_FIXTURES {
        let normalised = seed::extract_skill(&cache, &WikiTitle(title.to_owned()), None).unwrap();
        let text = seed::write_skill(&normalised.skill);
        let back: gwsim_data::Skill =
            ron::from_str(&text).unwrap_or_else(|e| panic!("{title}: {e}\n{text}"));
        assert_eq!(back, normalised.skill, "{title}");
    }
}

// -------------------------------------------------------------------- diff

fn run_diff(data: &Path, cache: &PageCache) -> diff::DiffReport {
    let core = CoreData::load(data.join("core")).ok();
    diff::diff(&DirSource::new(data), cache, core.as_ref(), true, true, &[]).unwrap()
}

#[test]
fn diff_straight_after_seeding_reports_no_changes() {
    let dir = TempDir::new("diff-clean");
    let data = scratch_data(dir.path());
    let cache = fixture_cache(dir.path());
    seed_everything(&cache, &data);

    let report = run_diff(&data, &cache);
    assert!(report.is_clean(), "{}", diff::render_markdown(&report));
    assert!(diff::render_markdown(&report).contains("No changes"));
    assert_eq!(diff::render_json(&report)["clean"], true);
}

#[test]
fn a_changed_energy_cost_is_a_field_change() {
    let dir = TempDir::new("diff-energy");
    let data = scratch_data(dir.path());
    let cache = fixture_cache(dir.path());
    seed_everything(&cache, &data);

    let edited = fixture("skill_basic.html").replace(
        "<li>10 <a href=\"/wiki/Energy\"",
        "<li>15 <a href=\"/wiki/Energy\"",
    );
    store(&cache, "Lorem Surge", &edited);

    let report = run_diff(&data, &cache);
    let entry = report
        .changed()
        .find(|e| e.title == "Lorem Surge")
        .expect("a change");
    assert_eq!(
        entry.changes,
        vec![Change::FieldChanged {
            path: "cost.energy".to_owned(),
            old: "10".to_owned(),
            new: "15".to_owned()
        }]
    );
    let markdown = diff::render_markdown(&report);
    assert!(markdown.contains("`cost.energy`: 10 → 15"), "{markdown}");
    assert!(markdown.contains("https://wiki.guildwars.com/wiki/Lorem_Surge"));
}

#[test]
fn a_changed_description_asks_for_re_translation() {
    let dir = TempDir::new("diff-description");
    let data = scratch_data(dir.path());
    let cache = fixture_cache(dir.path());
    seed_everything(&cache, &data);

    let edited =
        fixture("skill_basic.html").replace("Lorem skill text.", "Lorem skill text, revised.");
    store(&cache, "Lorem Surge", &edited);

    let report = run_diff(&data, &cache);
    let entry = report.changed().find(|e| e.title == "Lorem Surge").unwrap();
    assert_eq!(entry.changes, vec![Change::DescriptionChanged]);
    assert!(diff::render_markdown(&report).contains("Needs re-translation"));
}

#[test]
fn a_page_that_is_gone_is_a_removed_page() {
    let dir = TempDir::new("diff-removed");
    let data = scratch_data(dir.path());
    let cache = fixture_cache(dir.path());
    seed_everything(&cache, &data);

    cache
        .store_missing(&PageMeta {
            url: WikiTitle("Sit Chop".to_owned()).url(),
            title: "Sit Chop".to_owned(),
            canonical_title: None,
            fetched_at: "2026-09-24T00:00:00Z".to_owned(),
            checked_at: "2026-09-24T00:00:00Z".to_owned(),
            status: 404,
            etag: None,
            last_modified: None,
        })
        .unwrap();

    let report = run_diff(&data, &cache);
    let entry = report.changed().find(|e| e.title == "Sit Chop").unwrap();
    assert!(matches!(entry.changes[0], Change::RemovedPage { .. }));
}

#[test]
fn diff_never_writes_to_the_data_directory() {
    let dir = TempDir::new("diff-readonly");
    let data = scratch_data(dir.path());
    let cache = fixture_cache(dir.path());
    seed_everything(&cache, &data);

    let snapshot = |data: &Path| -> Vec<(PathBuf, std::time::SystemTime, u64)> {
        written_files(data)
            .into_iter()
            .map(|path| {
                let meta = fs::metadata(&path).unwrap();
                (path, meta.modified().unwrap(), meta.len())
            })
            .collect()
    };
    let before = snapshot(&data);
    run_diff(&data, &cache);
    assert_eq!(snapshot(&data), before);
}

// ------------------------------------------------------------------ crawl

#[test]
fn ext5_resume_after_interrupt() {
    let dir = TempDir::new("crawl-resume");
    let cache = PageCache::new(dir.path().join("cache"));
    let state = dir.path().join("crawl-state.json");
    let options = CrawlOptions {
        scope: Scope::Skills,
        only: vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
        discovery_only: false,
        dry_run: false,
        client: ClientOptions::default(),
    };
    let clock = FakeClock::default();

    // First run: A succeeds, B is refused with a 429, which stops the crawl.
    let first = FakeTransport::new().with_wiki_robots();
    first.respond("https://wiki.guildwars.com/wiki/A", Response::new(200, "a"));
    first.respond("https://wiki.guildwars.com/wiki/B", Response::new(429, ""));
    let mut output = Vec::new();
    let result = crawl::run(&options, &first, &clock, &cache, &state, &mut output);
    assert!(result.is_err());
    assert!(state.exists(), "the state is kept for a resume");

    // Second run: A is not asked for again.
    let second = FakeTransport::new().with_wiki_robots();
    second.respond("https://wiki.guildwars.com/wiki/B", Response::new(200, "b"));
    second.respond("https://wiki.guildwars.com/wiki/C", Response::new(200, "c"));
    let report = crawl::run(&options, &second, &clock, &cache, &state, &mut output).unwrap();

    let urls = second.urls();
    assert!(!urls.iter().any(|url| url.ends_with("/wiki/A")), "{urls:?}");
    assert_eq!(report.outcomes.len(), 2);
    assert!(!state.exists(), "a finished crawl clears its state");
    assert!(String::from_utf8(output).unwrap().contains("resuming"));
}

#[test]
fn a_dry_run_lists_urls_and_fetches_nothing() {
    let dir = TempDir::new("crawl-dry");
    let cache = PageCache::new(dir.path().join("cache"));
    let transport = FakeTransport::new().with_wiki_robots();
    let options = CrawlOptions {
        scope: Scope::Skills,
        only: Vec::new(),
        discovery_only: true,
        dry_run: true,
        client: ClientOptions::default(),
    };
    let mut output = Vec::new();
    crawl::run(
        &options,
        &transport,
        &FakeClock::default(),
        &cache,
        &dir.path().join("s.json"),
        &mut output,
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(
        transport.urls().is_empty(),
        "a dry run makes no request at all"
    );
    assert!(
        text.contains("https://wiki.guildwars.com/wiki/Skill_template_format/Skill_list"),
        "{text}"
    );
    assert!(text.contains("20 pages"), "{text}");
}

#[test]
fn a_fake_discovery_crawl_over_the_fixtures_completes() {
    let dir = TempDir::new("crawl-discovery");
    let cache = PageCache::new(dir.path().join("cache"));
    let transport = FakeTransport::new().with_wiki_robots();
    for title in gwsim_extractor::discovery::discovery_titles() {
        let body = match title.as_str() {
            "Skill template format/Skill list" => fixture("skill_list.html"),
            "List of PvE-only skills" => fixture("pve_only_list.html"),
            "List of mesmer skills" => fixture("profession_list.html"),
            t if t.starts_with("Guild Wars Wiki:") => fixture("game_integration.html"),
            _ => "<html><body></body></html>".to_owned(),
        };
        transport.respond(&title.url(), Response::new(200, body));
    }
    let options = CrawlOptions {
        scope: Scope::Skills,
        only: Vec::new(),
        discovery_only: true,
        dry_run: false,
        client: ClientOptions::default(),
    };
    let mut output = Vec::new();
    let report = crawl::run(
        &options,
        &transport,
        &FakeClock::default(),
        &cache,
        &dir.path().join("s.json"),
        &mut output,
    )
    .unwrap();
    assert_eq!(report.summary.fetched, 20);

    let merged = crawl::build_index(&cache).unwrap();
    assert_eq!(merged.skills.len(), 7);
}
