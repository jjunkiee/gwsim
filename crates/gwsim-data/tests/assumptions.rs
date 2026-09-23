//! T1.6.5: the assumptions register and coverage.

use std::path::PathBuf;

use gwsim_data::coverage::{AssumptionsUsed, Coverage};
use gwsim_data::dataset::DataSet;
use gwsim_data::source::{DirSource, MemSource};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn data() -> DataSet {
    DataSet::load(&DirSource::new(data_dir())).expect("data/ should load")
}

// ---------------------------------------------------------------- register

#[test]
fn the_register_holds_every_entry() {
    let data = data();
    assert_eq!(
        data.assumptions.len(),
        44,
        "DESIGN §21 lists 44 assumptions"
    );
}

#[test]
fn the_register_runs_from_a_001_to_a_044_with_no_gaps() {
    let data = data();
    let ids: Vec<String> = data
        .assumptions
        .all()
        .iter()
        .map(|entry| entry.id.to_string())
        .collect();

    let expected: Vec<String> = (1..=44).map(|n| format!("A-{n:03}")).collect();
    assert_eq!(
        ids, expected,
        "the register should be complete and in order"
    );
}

#[test]
fn every_entry_says_what_it_assumes_and_why() {
    let data = data();
    for entry in data.assumptions.all() {
        assert!(
            entry.statement.len() > 20,
            "{} has no real statement",
            entry.id
        );
        assert!(
            entry.rationale.len() > 20,
            "{} has no real rationale; the point of the register is the why",
            entry.id
        );
    }
}

#[test]
fn every_pending_entry_says_which_task_will_settle_it() {
    // A Pending assumption with no owner is one nobody will ever come back
    // to, which defeats the register.
    let data = data();
    for entry in data.assumptions.pending() {
        assert!(
            entry.notes.contains("T1.")
                || entry.notes.contains("T3.")
                || entry.notes.contains("T4.")
                || entry.notes.contains("WP"),
            "{} is Pending but names no task that will settle it: {:?}",
            entry.id,
            entry.notes
        );
    }
}

#[test]
fn the_assumptions_the_core_data_relies_on_have_values() {
    // core/ranges.ron and core/modes.ron already use these numbers, so a
    // Pending here would mean the data contradicts the register.
    let data = data();
    for id in ["A-005", "A-018", "A-024", "A-025", "A-020"] {
        let entry = data
            .assumptions
            .get(id.parse().unwrap())
            .unwrap_or_else(|| panic!("{id} should be in the register"));
        assert!(
            !entry.is_pending(),
            "{id} is used by core/*.ron but has no value"
        );
    }
}

#[test]
fn the_hard_mode_recharge_reduction_is_settled_at_zero() {
    // A-032. There is no number on the wiki to read. T4.2.1 settled it at 0%
    // (flagged on every hard-mode result) rather than leave foes undefined;
    // this test keeps changing it a deliberate act.
    let data = data();
    let entry = data
        .assumptions
        .get("A-032".parse().unwrap())
        .expect("A-032 should exist");
    assert!(!entry.is_pending());
    assert_eq!(
        entry.value,
        gwsim_data::assumptions::AssumptionValue::Percent(0.0)
    );
    assert!(entry.notes.contains("T4.2.1"), "{:?}", entry.notes);
}

#[test]
fn an_unknown_assumption_id_fails_validation() {
    let source = MemSource::default()
        .with(
            "assumptions.ron",
            r#"(
                provenance: (
                    sources: ["https://wiki.guildwars.com/wiki/Guild_Wars_Wiki"],
                    crawled: "2026-09-22",
                    review: Draft,
                ),
                assumptions: [
                    (
                        id: "A-001",
                        statement: "Something.",
                        value: Pending,
                        rationale: "Because.",
                        status: Assumed,
                    ),
                ],
            )"#,
        )
        .with(
            "skills/mesmer/test-skill.ron",
            r#"(
                id: 1,
                name: "Test Skill",
                wiki: "Test Skill",
                profession: Some(Mesmer),
                kind: Spell,
                campaign: Prophecies,
                cost: (energy: 5),
                activation: 1.0,
                recharge: 5.0,
                target: Foe,
                encoding: Some((effects: [])),
                provenance: (
                    sources: ["https://wiki.guildwars.com/wiki/Test_Skill"],
                    crawled: "2026-09-22",
                    review: Draft,
                    assumptions: ["A-999"],
                ),
            )"#,
        );

    let problems = DataSet::load(&source).expect_err("should be rejected");
    assert!(
        problems
            .errors()
            .any(|problem| problem.message.contains("A-999")),
        "expected a dangling assumption error, got:\n{problems}"
    );
}

#[test]
fn a_duplicate_assumption_id_is_rejected() {
    let entry = |id: &str| {
        format!(
            r#"(id: "{id}", statement: "Something.", value: Pending, rationale: "Because.", status: Assumed)"#
        )
    };
    let source = MemSource::default().with(
        "assumptions.ron",
        format!(
            r#"(
                provenance: (
                    sources: ["https://wiki.guildwars.com/wiki/Guild_Wars_Wiki"],
                    crawled: "2026-09-22",
                    review: Draft,
                ),
                assumptions: [{}, {}],
            )"#,
            entry("A-001"),
            entry("A-001")
        ),
    );

    let problems = DataSet::load(&source).expect_err("should be rejected");
    assert!(
        problems
            .errors()
            .any(|problem| problem.message.contains("twice")),
        "{problems}"
    );
}

#[test]
fn an_out_of_order_register_is_rejected() {
    let entry = |id: &str| {
        format!(
            r#"(id: "{id}", statement: "Something.", value: Pending, rationale: "Because.", status: Assumed)"#
        )
    };
    let source = MemSource::default().with(
        "assumptions.ron",
        format!(
            r#"(
                provenance: (
                    sources: ["https://wiki.guildwars.com/wiki/Guild_Wars_Wiki"],
                    crawled: "2026-09-22",
                    review: Draft,
                ),
                assumptions: [{}, {}],
            )"#,
            entry("A-005"),
            entry("A-001")
        ),
    );

    let problems = DataSet::load(&source).expect_err("should be rejected");
    assert!(
        problems
            .errors()
            .any(|problem| problem.message.contains("id order")),
        "{problems}"
    );
}

// --------------------------------------------------------------- recording

#[test]
fn the_recorded_set_is_exactly_the_ids_that_were_read() {
    let mut used = AssumptionsUsed::new();
    used.record("A-030".parse().unwrap());
    used.record("A-012".parse().unwrap());
    used.record("A-030".parse().unwrap());

    assert_eq!(used.len(), 2);
    assert!(used.contains("A-012".parse().unwrap()));
    assert!(used.contains("A-030".parse().unwrap()));
    assert!(!used.contains("A-005".parse().unwrap()));
}

#[test]
fn a_recorded_set_reads_back_in_a_stable_order() {
    // Two runs that touched the same assumptions must report the same list,
    // whatever order they touched them in (§10.13).
    let mut first = AssumptionsUsed::new();
    first.record_all(["A-030".parse().unwrap(), "A-005".parse().unwrap()]);
    let mut second = AssumptionsUsed::new();
    second.record_all(["A-005".parse().unwrap(), "A-030".parse().unwrap()]);
    assert_eq!(first, second);
}

#[test]
fn every_recorded_id_resolves_to_a_statement() {
    // The report layer turns ids into sentences, so an id nobody can look up
    // would produce a report with a hole in it.
    let data = data();
    let mut used = AssumptionsUsed::new();
    used.record_all(["A-005".parse().unwrap(), "A-033".parse().unwrap()]);

    for id in used.ids() {
        let entry = data
            .assumptions
            .get(*id)
            .unwrap_or_else(|| panic!("{id} should resolve"));
        assert!(!entry.statement.is_empty());
    }
}

// ---------------------------------------------------------------- coverage

#[test]
fn coverage_of_the_repositorys_data_counts_against_the_whole_game() {
    // Since T2.3.6 the skill index gives every percentage a real
    // denominator, and every indexed skill is either seeded or not started.
    let data = data();
    let coverage = Coverage::compute(&data);
    assert!(!coverage.is_empty());
    assert!(coverage.denominator_is_complete);
    let index = data
        .skill_index
        .as_ref()
        .expect("data/skills/index.ron exists");
    assert_eq!(coverage.total.total(), index.value.skills.len());
    assert_eq!(
        coverage.total.with_files() + coverage.total.not_started,
        index.value.skills.len()
    );
}

#[test]
fn coverage_counts_skills_by_profession_and_status() {
    let skill = |name: &str, profession: &str, status: &str| {
        let slug = name.to_lowercase().replace(' ', "-");
        let encoding = if status == "NumbersOnly" {
            "encoding: None,"
        } else {
            "encoding: Some((effects: [])),"
        };
        (
            format!("skills/{}/{slug}.ron", profession.to_lowercase()),
            format!(
                r#"(
                    id: {},
                    name: "{name}",
                    wiki: "{name}",
                    profession: Some({profession}),
                    kind: Spell,
                    campaign: Prophecies,
                    cost: (energy: 5),
                    activation: 1.0,
                    recharge: 5.0,
                    target: Foe,
                    {encoding}
                    provenance: (
                        sources: ["https://wiki.guildwars.com/wiki/{name}"],
                        crawled: "2026-09-22",
                        review: {status},
                        reviewed_by: {},
                    ),
                )"#,
                slug.len() * 7 + name.len(),
                if status == "Reviewed" {
                    "Some(\"owner\")"
                } else {
                    "None"
                }
            ),
        )
    };

    let mut source = MemSource::default();
    for (path, contents) in [
        skill("Alpha One", "Mesmer", "Reviewed"),
        skill("Beta Two", "Mesmer", "Draft"),
        skill("Gamma Three", "Mesmer", "NumbersOnly"),
        skill("Delta Four", "Necromancer", "Draft"),
    ] {
        source = source.with(path, contents);
    }

    let data = DataSet::load(&source).expect("the fixture should load");
    let coverage = Coverage::compute(&data);

    let mesmer = coverage.by_profession[&Some(gwsim_data::core::Profession::Mesmer)];
    assert_eq!(mesmer.reviewed, 1);
    assert_eq!(mesmer.draft, 1);
    assert_eq!(mesmer.numbers_only, 1);
    assert_eq!(mesmer.total(), 3);
    assert!((mesmer.encoded_percent() - 66.666).abs() < 0.01);

    assert_eq!(coverage.total.total(), 4);
    assert_eq!(coverage.total.draft, 2);
}

#[test]
fn coverage_lists_foe_skills_that_have_no_file() {
    let source = MemSource::default().with(
        "creatures/foes/kournan/kournan-seer.ron",
        r#"(
            name: "Kournan Seer",
            wiki: "Kournan Seer",
            affiliation: "kournan",
            species: "Human",
            professions: (Mesmer, None),
            level: (nm: 20, hm: Some(26)),
            attributes: (nm: []),
            skills: [(skill: Slug("power-spike"))],
            provenance: (
                sources: ["https://wiki.guildwars.com/wiki/Kournan_Seer"],
                crawled: "2026-09-22",
                review: NumbersOnly,
            ),
        )"#,
    );

    // The tree does not load, because a dangling reference is an error. The
    // coverage report is for a tree that *does* load, so the interesting
    // check is that the error names the skill.
    let problems = DataSet::load(&source).expect_err("should be rejected");
    assert!(
        problems
            .errors()
            .any(|problem| problem.message.contains("power-spike")),
        "{problems}"
    );
}

#[test]
fn coverage_flags_foes_whose_skills_are_seeded_but_not_encoded() {
    let source = MemSource::default()
        .with(
            "skills/mesmer/power-spike.ron",
            r#"(
                id: 23,
                name: "Power Spike",
                wiki: "Power Spike",
                profession: Some(Mesmer),
                kind: Spell,
                campaign: Prophecies,
                cost: (energy: 5),
                activation: 0.25,
                recharge: 12.0,
                target: Foe,
                encoding: None,
                provenance: (
                    sources: ["https://wiki.guildwars.com/wiki/Power_Spike"],
                    crawled: "2026-09-22",
                    review: NumbersOnly,
                ),
            )"#,
        )
        .with(
            "creatures/foes/kournan/kournan-seer.ron",
            r#"(
                name: "Kournan Seer",
                wiki: "Kournan Seer",
                affiliation: "kournan",
                species: "Human",
                professions: (Mesmer, None),
                level: (nm: 20, hm: Some(26)),
                attributes: (nm: []),
                skills: [(skill: Slug("power-spike"))],
                provenance: (
                    sources: ["https://wiki.guildwars.com/wiki/Kournan_Seer"],
                    crawled: "2026-09-22",
                    review: NumbersOnly,
                ),
            )"#,
        );

    let data = DataSet::load(&source).expect("the fixture should load");
    let coverage = Coverage::compute(&data);

    let seer: gwsim_data::ids::Slug = "kournan-seer".parse().unwrap();
    assert!(
        coverage.foes_with_unencoded_skills.contains_key(&seer),
        "the seer's only skill is NumbersOnly, so it should be listed"
    );
    assert_eq!(
        coverage.foes_with_unencoded_skills[&seer]
            .iter()
            .map(|slug| slug.to_string())
            .collect::<Vec<_>>(),
        ["power-spike"]
    );
}
