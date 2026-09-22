//! T1.7.1: how long does loading `data/` take at full coverage?
//!
//! `data/` holds a dozen files today, so timing it says nothing about the
//! tree the project will have. This builds a synthetic tree the size DESIGN
//! expects at full coverage — about 2,000 skills plus foes — and times the
//! WP1.2 loader against it.
//!
//! Run with `cargo test --release --test pack_scale -- --ignored --nocapture`.
//! Ignored by default because it is a measurement, not an assertion.

use std::time::Instant;

use gwsim_data::dataset::DataSet;
use gwsim_data::source::MemSource;

/// Roughly what full coverage looks like: every player skill, plus monster
/// skills and a foe roster.
const SKILLS: usize = 2_000;
const FOES: usize = 400;

fn skill(index: usize) -> (String, String) {
    let professions = [
        "Warrior",
        "Ranger",
        "Monk",
        "Necromancer",
        "Mesmer",
        "Elementalist",
        "Assassin",
        "Ritualist",
        "Paragon",
        "Dervish",
    ];
    let profession = professions[index % professions.len()];
    let folder = profession.to_lowercase();
    let slug = format!("skill-{index}");

    let contents = format!(
        r#"(
    id: {index},
    name: "Skill {index}",
    wiki: "Skill {index}",
    profession: Some({profession}),
    attribute: Some(DominationMagic),
    kind: Spell,
    campaign: Prophecies,
    cost: (energy: 10),
    activation: 1.0,
    recharge: 15.0,
    target: Foe,
    range: Some(Casting),
    extracted: (
        scaled: [
            (label: "damage", r0: 10, r12: 66, r15: 80),
            (label: "duration", r0: 5, r12: 13, r15: 15),
        ],
    ),
    encoding: Some((
        effects: [
            Damage(to: TargetFoe, amount: Scaled(10, 80)),
            Damage(to: Secondary(of: Nearby(TargetFoe), factor: 0.75), amount: Scaled(10, 80)),
            Control(If(
                condition: Hexed,
                of: Some(TargetFoe),
                then: [Damage(to: TargetFoe, amount: Scaled(5, 40))],
            )),
        ],
        effect_defs: [
            (
                id: "effect-{index}",
                kind: Hex,
                while_active: [
                    ModifyStat(to: Target, stat: HealthRegeneration, amount: Scaled(-1, -3)),
                ],
            ),
        ],
        roles: [Damage, Aoe],
    )),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Skill_{index}"],
        crawled: "2026-09-22",
        review: Draft,
        notes: "Synthetic, for the T1.7.1 measurement only.",
    ),
)"#
    );

    // The attribute is wrong for most professions, which the validator will
    // not mind: nothing here checks that an attribute belongs to the skill's
    // profession. The shape and size are what is being measured.
    (format!("skills/{folder}/{slug}.ron"), contents)
}

fn foe(index: usize) -> (String, String) {
    let slug = format!("foe-{index}");
    let contents = format!(
        r#"(
    name: "Foe {index}",
    wiki: "Foe {index}",
    affiliation: "synthetic",
    species: "Human",
    traits: [Fleshy],
    professions: (Mesmer, None),
    level: (nm: 20, hm: Some(26)),
    attributes: (nm: [(DominationMagic, 15), (InspirationMagic, 14)]),
    skills: [
        (skill: Slug("skill-{}")),
        (skill: Slug("skill-{}")),
    ],
    armor: (default: Some(60), level_context: Some(20)),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Foe_{index}"],
        crawled: "2026-09-22",
        review: NumbersOnly,
    ),
)"#,
        index % SKILLS,
        (index * 3) % SKILLS
    );
    (format!("creatures/foes/synthetic/{slug}.ron"), contents)
}

fn build_tree() -> MemSource {
    let mut files: Vec<(String, String)> = Vec::with_capacity(SKILLS + FOES);
    for index in 0..SKILLS {
        files.push(skill(index));
    }
    for index in 0..FOES {
        files.push(foe(index));
    }
    MemSource::new(files)
}

#[test]
#[ignore = "a measurement, not an assertion; run with --ignored --nocapture"]
fn measure_full_coverage_load() {
    let built = Instant::now();
    let source = build_tree();
    let bytes: usize = source
        .files()
        .iter()
        .map(|(path, contents)| path.len() + contents.len())
        .sum();
    println!(
        "built {} files, {:.1} MB, in {:?}",
        source.files().len(),
        bytes as f64 / 1_048_576.0,
        built.elapsed()
    );

    // Load it a few times, since the first may pay for warm-up.
    let mut best = None::<std::time::Duration>;
    for round in 1..=3 {
        let start = Instant::now();
        let result = DataSet::load(&source);
        let elapsed = start.elapsed();

        match &result {
            Ok(data) => println!("  round {round}: {elapsed:?}, {:?}", data.counts()),
            Err(problems) => println!(
                "  round {round}: {elapsed:?}, {} problems",
                problems.0.len()
            ),
        }
        best = Some(best.map_or(elapsed, |best: std::time::Duration| best.min(elapsed)));
    }

    println!("best: {:?}", best.unwrap());
    println!(
        "\nThis is the number T1.7.1's decision rests on: if loading RON at \
         full coverage is fast enough, embedding the text and parsing at \
         start-up is simpler than embedding a binary serialisation."
    );
}
