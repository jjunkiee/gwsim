//! T1.2.7, T1.2.8 and T1.2.10: a data tree loads, and a broken one says why.
//!
//! **Fixtures are built in memory, not on disk.** The plan asked for
//! `tests/fixtures/valid/` and one directory per defect. Using [`MemSource`]
//! instead buys two things that matter more than the layout: every invalid
//! case is written as *the valid tree with one thing changed*, so the defect
//! is visible in the test rather than buried in a directory diff; and there is
//! no chance of fourteen near-identical trees drifting apart as the schemas
//! move. `MemSource` exists for exactly this (§T1.2.7).
//!
//! The snapshots are the "good errors" standard: each must name the file, the
//! place or field, and what to do about it.

use gwsim_data::dataset::DataSet;
use gwsim_data::error::DataErrors;
use gwsim_data::source::MemSource;

// --------------------------------------------------------------- the fixture

const ASSUMPTIONS: &str = r#"(
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Guild_Wars_Wiki"],
        crawled: "2026-09-22",
        review: Draft,
    ),
    assumptions: [
        (
            id: "A-004",
            statement: "Foe weapon damage and attack interval.",
            value: Pending,
            rationale: "Creature pages do not record weapons.",
            status: Assumed,
        ),
        (
            id: "A-005",
            statement: "Missing hard-mode attribute ranks are normal mode plus five.",
            value: Number(5.0),
            rationale: "The wiki gives hard-mode ranks for very few foes.",
            status: Assumed,
        ),
        (
            id: "A-006",
            statement: "The Kournan patrol is one of each of eight types.",
            value: Text("one of each"),
            rationale: "No composition is published.",
            status: Assumed,
        ),
        (
            id: "A-008",
            statement: "Foes start about 1800 gwinches ahead of the party.",
            value: Gwinches(1800.0),
            rationale: "No positions are published.",
            status: Assumed,
        ),
    ],
)"#;

const ENERGY_SURGE: &str = r#"(
    id: 1234,
    name: "Energy Surge",
    wiki: "Energy Surge",
    profession: Some(Mesmer),
    attribute: Some(DominationMagic),
    kind: Spell,
    elite: true,
    campaign: Prophecies,
    cost: (energy: 10),
    activation: 0.25,
    recharge: 20.0,
    target: Foe,
    range: Some(Casting),
    extracted: (
        scaled: [
            (label: "energy lost", r0: 1, r12: 8, r15: 10),
        ],
    ),
    encoding: Some((
        effects: [LoseEnergy(to: TargetFoe, amount: Scaled(1, 10))],
        roles: [Damage, EnergyManagement],
    )),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Energy_Surge"],
        crawled: "2026-09-22",
        review: Draft,
    ),
)"#;

const POWER_SPIKE: &str = r#"(
    id: 5678,
    name: "Power Spike",
    wiki: "Power Spike",
    profession: Some(Mesmer),
    attribute: Some(DominationMagic),
    kind: Spell,
    campaign: Prophecies,
    cost: (energy: 10),
    activation: 0.25,
    recharge: 20.0,
    target: Foe,
    range: Some(Casting),
    extracted: (
        scaled: [
            (label: "damage", r0: 30, r12: 78, r15: 90),
        ],
    ),
    encoding: Some((
        effects: [Interrupt(to: TargetFoe)],
        roles: [Interrupt, Damage],
    )),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Power_Spike"],
        crawled: "2026-09-22",
        review: Draft,
    ),
)"#;

const KOURNAN_SEER: &str = r#"(
    name: "Kournan Seer",
    wiki: "Kournan Seer",
    affiliation: "kournan",
    species: "Human",
    traits: [Fleshy],
    professions: (Mesmer, None),
    level: (nm: 20, hm: Some(26)),
    attributes: (
        nm: [(DominationMagic, 15), (InspirationMagic, 14)],
        hm: Some([(DominationMagic, 20), (InspirationMagic, 14)]),
    ),
    skills: [
        (skill: Slug("energy-surge")),
        (skill: Slug("power-spike")),
    ],
    armor: (default: Some(60), level_context: Some(20)),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Kournan_Seer"],
        crawled: "2026-09-22",
        review: NumbersOnly,
        assumptions: ["A-004"],
    ),
)"#;

const KOURNAN_PATROL: &str = r#"(
    name: "Kournan patrol",
    area: "Vehtendi Valley",
    campaign: Nightfall,
    groups: [
        (
            foes: [(foe: "kournan-seer", count: 1)],
            formation: Cluster(radius: 150.0),
            position: (x: 0.0, y: 1800.0),
        ),
    ],
    party_start: (x: 0.0, y: 0.0),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Vehtendi_Valley"],
        crawled: "2026-09-22",
        review: Draft,
        assumptions: ["A-006", "A-008"],
    ),
)"#;

const SITUATION: &str = r#"(
    name: "Kournan patrol HM",
    encounters: Single("kournan-patrol"),
    mode: (hard_mode: true),
    party_size: 8,
)"#;

const SITUATION_SET: &str = r#"(
    name: "Kournan mix",
    entries: [(situation: "kournan-patrol-hm", weight: 1.0)],
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Vehtendi_Valley"],
        crawled: "2026-09-22",
        review: Draft,
    ),
)"#;

const BENCHMARK: &str = r#"(
    name: "7 Hero Mesmerway",
    source_url: "https://gwpvx.fandom.com/wiki/Build:Team_-_7_Hero_Mesmerway",
    snapshot_date: Some("2026-07-27"),
    slots: [(kind: Human, skill_code: "OQBTAUBPQaJ4EY6x0BAAAAAAuE")],
    provenance: (
        sources: ["https://gwpvx.fandom.com/wiki/Build:Team_-_7_Hero_Mesmerway"],
        crawled: "2026-09-22",
        review: Draft,
    ),
)"#;

const SKILL_PATH: &str = "skills/mesmer/energy-surge.ron";
const FOE_PATH: &str = "creatures/foes/kournan/kournan-seer.ron";
const ENCOUNTER_PATH: &str = "encounters/curated/nightfall/vehtendi-valley/kournan-patrol.ron";

/// A tree that loads cleanly.
fn valid() -> MemSource {
    MemSource::default()
        .described_as("the valid fixture")
        .with("ATTRIBUTION.md", "# Attribution\n")
        .with("assumptions.ron", ASSUMPTIONS)
        .with(SKILL_PATH, ENERGY_SURGE)
        .with("skills/mesmer/power-spike.ron", POWER_SPIKE)
        .with(FOE_PATH, KOURNAN_SEER)
        .with(ENCOUNTER_PATH, KOURNAN_PATROL)
        .with("situations/kournan-patrol-hm.ron", SITUATION)
        .with("situation_sets/kournan-mix.ron", SITUATION_SET)
        .with("benchmarks/7-hero-mesmerway.ron", BENCHMARK)
}

/// Loads a tree that is expected to fail, and returns the report.
fn report(source: &MemSource) -> String {
    match DataSet::load(source) {
        Ok(data) => panic!(
            "expected this tree to be rejected, but it loaded: {:?}",
            data.counts()
        ),
        Err(problems) => problems.to_string(),
    }
}

/// Loads a tree that is expected to succeed.
fn load(source: &MemSource) -> DataSet {
    match DataSet::load(source) {
        Ok(data) => data,
        Err(problems) => panic!("expected this tree to load, but:\n{problems}"),
    }
}

// ------------------------------------------------------------- the good case

#[test]
fn the_valid_tree_loads() {
    let data = load(&valid());
    let counts = data.counts();
    assert_eq!(counts.skills, 2);
    assert_eq!(counts.foes, 1);
    assert_eq!(counts.encounters, 1);
    assert_eq!(counts.situations, 1);
    assert_eq!(counts.situation_sets, 1);
    assert_eq!(counts.benchmarks, 1);
    assert_eq!(counts.assumptions, 4);
}

#[test]
fn markdown_and_licence_files_are_not_data() {
    let source = valid()
        .with("LICENSE", "GPL-3.0-or-later")
        .with("skills/README.md", "# Skills\n");
    let data = load(&source);
    assert_eq!(data.counts().skills, 2);
}

#[test]
fn core_files_are_left_to_the_core_loader() {
    // CoreData checks these against the enums in code, so the tree loader
    // must pass over them rather than reject them as unknown paths.
    let source = valid().with("core/ranges.ron", "(this is not checked here)");
    load(&source);
}

#[test]
fn entities_are_indexed_by_slug_and_by_template_id() {
    let data = load(&valid());
    let slug = "energy-surge".parse().unwrap();
    assert!(data.skill(&slug).is_some());
    assert_eq!(
        data.skill_by_id(gwsim_data::SkillId(1234)).map(|s| &s.name),
        Some(&"Energy Surge".to_owned())
    );
    assert!(data.skill_by_id(gwsim_data::SkillId(9999)).is_none());
}

#[test]
fn a_numbers_only_foe_skill_is_a_warning_not_an_error() {
    // The Kournan Seer is NumbersOnly, but its *skills* are Draft, so nothing
    // warns. Downgrading a skill should produce a warning and still load.
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE
            .replace("review: Draft", "review: NumbersOnly")
            .replace(
                r#"encoding: Some((
        effects: [LoseEnergy(to: TargetFoe, amount: Scaled(1, 10))],
        roles: [Damage, EnergyManagement],
    )),"#,
                "encoding: None,",
            ),
    );
    let data = load(&source);
    let warnings = data.warnings();
    assert_eq!(warnings.error_count(), 0);
    assert!(
        warnings
            .warnings()
            .any(|problem| problem.message.contains("NumbersOnly")),
        "expected a warning about unencoded foe skills, got:\n{warnings}"
    );
}

#[test]
fn a_pending_assumption_is_a_warning() {
    // The Kournan Seer cites A-004, which has no value yet.
    let data = load(&valid());
    assert!(
        data.warnings()
            .warnings()
            .any(|problem| problem.message.contains("A-004")),
        "expected a warning about the pending assumption, got:\n{}",
        data.warnings()
    );
}

// ----------------------------------------------------------- the defect cases

#[test]
fn bad_ron_syntax() {
    let source = valid().with(SKILL_PATH, "(\n    id: 1234,\n    name: \"Energy Surge\"\n");
    insta::assert_snapshot!(report(&source));
}

#[test]
fn an_unknown_field() {
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace("recharge: 20.0", "recharge_time: 20.0"),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn an_unknown_enum_variant() {
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace(
            "attribute: Some(DominationMagic)",
            "attribute: Some(Domination)",
        ),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_missing_required_field() {
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace("    campaign: Prophecies,\n", ""),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn an_unknown_assumption_id() {
    let source = valid().with(
        FOE_PATH,
        KOURNAN_SEER.replace(r#"assumptions: ["A-004"]"#, r#"assumptions: ["A-404"]"#),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_dangling_foe_skill() {
    let source = valid().with(
        FOE_PATH,
        KOURNAN_SEER.replace(r#"Slug("power-spike")"#, r#"Slug("power-spik")"#),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_dangling_encounter_foe() {
    let source = valid().with(
        ENCOUNTER_PATH,
        KOURNAN_PATROL.replace(r#"foe: "kournan-seer""#, r#"foe: "kournan-sear""#),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_duplicate_skill_id() {
    // Two files claiming template id 1234. The message must name both.
    let source = valid().with(
        "skills/mesmer/power-spike.ron",
        POWER_SPIKE.replace("id: 5678", "id: 1234"),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_file_name_that_does_not_match_its_slug() {
    let source = valid()
        .without(SKILL_PATH)
        .with("skills/mesmer/energy_surge.ron", ENERGY_SURGE);
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_skill_in_the_wrong_professions_folder() {
    let source = valid()
        .without(SKILL_PATH)
        .with("skills/necromancer/energy-surge.ron", ENERGY_SURGE);
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_foe_in_the_wrong_affiliation_folder() {
    let source = valid()
        .without(FOE_PATH)
        .with("creatures/foes/margonite/kournan-seer.ron", KOURNAN_SEER);
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_scaled_value_missing_an_endpoint() {
    // All three rank points are required fields, so this is caught by the
    // schema rather than by a consistency rule. That is the stronger place
    // for it: the file cannot even be built wrong.
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace(
            r#"(label: "energy lost", r0: 1, r12: 8, r15: 10),"#,
            r#"(label: "energy lost", r0: 1, r15: 10),"#,
        ),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn an_r12_that_contradicts_the_scaling_formula() {
    // round(1 + 12 * (10 - 1) / 15) = round(8.2) = 8, not 5.
    let source = valid().with(SKILL_PATH, ENERGY_SURGE.replace("r12: 8", "r12: 5"));
    insta::assert_snapshot!(report(&source));
}

#[test]
fn an_r12_mismatch_is_only_a_warning_when_special_rounding_is_declared() {
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace(
            r#"(label: "energy lost", r0: 1, r12: 8, r15: 10),"#,
            r#"(label: "energy lost", r0: 1, r12: 5, r15: 10, special_rounding: true),"#,
        ),
    );
    let data = load(&source);
    assert_eq!(data.warnings().error_count(), 0);
    assert_eq!(data.warnings().warnings().count(), 2); // this, plus A-004
}

#[test]
fn a_numbers_only_skill_with_an_encoding() {
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace("review: Draft", "review: NumbersOnly"),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_draft_skill_without_an_encoding() {
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace(
            r#"encoding: Some((
        effects: [LoseEnergy(to: TargetFoe, amount: Scaled(1, 10))],
        roles: [Damage, EnergyManagement],
    )),"#,
            "encoding: None,",
        ),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_reviewed_skill_without_a_reviewer() {
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace("review: Draft", "review: Reviewed"),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_foe_missing_its_hard_mode_level() {
    let source = valid().with(
        FOE_PATH,
        KOURNAN_SEER.replace("level: (nm: 20, hm: Some(26))", "level: (nm: 20)"),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_pve_only_skill_outside_the_common_folder() {
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace("elite: true,", "elite: true,\n    pve_only: true,"),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_title_track_without_pve_only() {
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace(
            "elite: true,",
            "elite: true,\n    title_track: Some(Asura),",
        ),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_file_somewhere_the_layout_does_not_allow() {
    let source = valid().with("skills/energy-surge.ron", ENERGY_SURGE);
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_dangling_situation_encounter() {
    let source = valid().with(
        "situations/kournan-patrol-hm.ron",
        SITUATION.replace(
            r#"Single("kournan-patrol")"#,
            r#"Single("kournan-patroll")"#,
        ),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_dangling_situation_set_entry() {
    let source = valid().with(
        "situation_sets/kournan-mix.ron",
        SITUATION_SET.replace(r#"situation: "kournan-patrol-hm""#, r#"situation: "nope""#),
    );
    insta::assert_snapshot!(report(&source));
}

#[test]
fn a_variant_an_encounter_asks_for_but_the_foe_does_not_have() {
    let source = valid().with(
        ENCOUNTER_PATH,
        KOURNAN_PATROL.replace(
            r#"(foe: "kournan-seer", count: 1)"#,
            r#"(foe: "kournan-seer", variant: Some("axe"), count: 1)"#,
        ),
    );
    insta::assert_snapshot!(report(&source));
}

// ------------------------------------------------------ several at once

#[test]
fn three_broken_files_report_three_problems() {
    // T1.2.7's done criterion: the loader must not stop at the first bad file.
    let source = valid()
        .with(
            SKILL_PATH,
            ENERGY_SURGE.replace("recharge: 20.0", "recharge_time: 20.0"),
        )
        .with(
            FOE_PATH,
            KOURNAN_SEER.replace("level: (nm: 20, hm: Some(26))", "level: (nm: 20)"),
        )
        .with(
            ENCOUNTER_PATH,
            KOURNAN_PATROL.replace(r#"foe: "kournan-seer""#, r#"foe: "nobody""#),
        );

    let problems = DataSet::load(&source).expect_err("should be rejected");
    let files: std::collections::BTreeSet<&str> = problems
        .errors()
        .map(|problem| problem.file.as_str())
        .collect();
    assert!(
        files.len() >= 3,
        "expected problems in at least three files, got {files:?}:\n{problems}"
    );
    insta::assert_snapshot!(problems.to_string());
}

// ------------------------------------------------------------- handlers

#[test]
fn a_handler_name_is_checked_against_the_registry() {
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace(
            "roles: [Damage, EnergyManagement],",
            r#"roles: [Damage, EnergyManagement], handler: Some((name: "energy_surge")),"#,
        ),
    );
    let data = load(&source);

    let known: &[&str] = &["energy_surge", "panic"];
    let problems = gwsim_data::checks::check_handlers(&data, &known);
    assert!(problems.is_empty(), "{problems}");

    let unknown: &[&str] = &["panic", "mistrust"];
    let problems = gwsim_data::checks::check_handlers(&data, &unknown);
    assert_eq!(problems.error_count(), 1);
    insta::assert_snapshot!(problems.to_string());
}

#[test]
fn an_empty_tree_is_not_an_error_in_itself() {
    // Loading nothing is legal; it is the checks that decide whether the
    // result is usable. WP1.7's embedded pack relies on this.
    let data = load(&MemSource::default());
    assert_eq!(data.counts(), Default::default());
}

#[test]
fn reports_are_stable_between_runs() {
    let source = valid().with(
        SKILL_PATH,
        ENERGY_SURGE.replace("recharge: 20.0", "recharge_time: 20.0"),
    );
    let first = report(&source);
    let second = report(&source);
    assert_eq!(first, second, "the report changed between identical runs");
}

/// Keeps `DataErrors` in the test's public surface, so the import above is
/// not merely decorative.
#[test]
fn a_clean_tree_reports_nothing_fatal() {
    let data = load(&valid());
    let warnings: &DataErrors = data.warnings();
    assert!(!warnings.is_fatal());
}
