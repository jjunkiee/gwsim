//! T1.4.3 and T1.4.8: one test per legality rule, and the foe stat rules.

use gwsim_data::build::{ArmorPiece, Build, LegalityError, SlotKind};
use gwsim_data::core::{ArmorSlot, Attribute, CoreData, DamageType, HmLevelKind, Profession};
use gwsim_data::dataset::DataSet;
use gwsim_data::derived::{foe_armor, foe_attributes, foe_energy, foe_level, mapped_hm_level};
use gwsim_data::foe::Foe;
use gwsim_data::ids::SkillId;
use gwsim_data::source::{DirSource, MemSource};

fn core() -> CoreData {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/core");
    CoreData::load(dir).expect("data/core should load")
}

/// A skill file, parameterised by the things legality cares about.
fn skill(id: u16, name: &str, profession: &str, elite: bool, pve_only: bool) -> String {
    let profession = if profession.is_empty() {
        "None".to_owned()
    } else {
        format!("Some({profession})")
    };
    format!(
        r#"(
    id: {id},
    name: "{name}",
    wiki: "{name}",
    profession: {profession},
    kind: Spell,
    elite: {elite},
    pve_only: {pve_only},
    campaign: Prophecies,
    cost: (energy: 10),
    activation: 1.0,
    recharge: 10.0,
    target: Foe,
    encoding: Some((effects: [])),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/{name}"],
        crawled: "2026-09-22",
        review: Draft,
    ),
)"#
    )
}

/// A data set holding the real items plus a handful of test skills.
fn data() -> DataSet {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let real = DirSource::new(&repo);

    let mut source = MemSource::default().described_as("legality fixtures");
    // The real items and core files, but not the seeded skills, foes or the
    // skill index: the test skills below use invented ids that would clash
    // with real ones.
    for (path, contents) in real_files(&real) {
        if path.starts_with("skills/") || path.starts_with("creatures/") {
            continue;
        }
        source = source.with(path, contents);
    }

    source = source
        .with(
            "skills/mesmer/mesmer-spell.ron",
            skill(100, "Mesmer Spell", "Mesmer", false, false),
        )
        .with(
            "skills/mesmer/mesmer-elite.ron",
            skill(101, "Mesmer Elite", "Mesmer", true, false),
        )
        .with(
            "skills/mesmer/mesmer-elite-two.ron",
            skill(102, "Mesmer Elite Two", "Mesmer", true, false),
        )
        .with(
            "skills/necromancer/necro-spell.ron",
            skill(200, "Necro Spell", "Necromancer", false, false),
        )
        .with(
            "skills/common/common-spell.ron",
            skill(300, "Common Spell", "", false, false),
        );

    for index in 0..4 {
        source = source.with(
            format!("skills/common/pve-skill-{index}.ron"),
            skill(400 + index, &format!("Pve Skill {index}"), "", false, true),
        );
    }

    DataSet::load(&source).expect("the fixture tree should load")
}

/// Copies the repository's real data files into a memory source.
fn real_files(source: &DirSource) -> Vec<(String, String)> {
    use gwsim_data::source::DataSource;
    source
        .list()
        .expect("data/ should list")
        .into_iter()
        .filter_map(|path| source.read(&path).ok().map(|text| (path, text)))
        .collect()
}

fn mesmer() -> Build {
    let mut build = Build::new(Profession::Mesmer);
    build.armor = ArmorSlot::ALL.map(|slot| ArmorPiece {
        slot,
        insignia: None,
        rune: None,
    });
    build
}

fn has<F>(problems: &[LegalityError], predicate: F) -> bool
where
    F: Fn(&LegalityError) -> bool,
{
    problems.iter().any(predicate)
}

// ------------------------------------------------------------ the good case

#[test]
fn a_sound_build_reports_nothing() {
    let data = data();
    let mut build = mesmer();
    build
        .attribute_points
        .insert(Attribute::DominationMagic, 12);
    build.headgear_attribute = Some(Attribute::DominationMagic);
    build.skills[0] = Some(SkillId(100));
    build.skills[1] = Some(SkillId(101));
    build.armor[0].rune = Some("superior-domination-magic".parse().unwrap());
    build.armor[0].insignia = Some("prodigys".parse().unwrap());

    let problems = build.check(&data, SlotKind::Human);
    assert!(problems.is_empty(), "{problems:?}");
    assert!(build.is_legal(&data, SlotKind::Human));
}

// -------------------------------------------------------------- one per rule

#[test]
fn the_secondary_may_not_match_the_primary() {
    let data = data();
    let mut build = mesmer();
    build.secondary = Some(Profession::Mesmer);
    let problems = build.check(&data, SlotKind::Human);
    assert!(has(&problems, |p| matches!(
        p,
        LegalityError::SecondaryMatchesPrimary(Profession::Mesmer)
    )));
}

#[test]
fn a_skill_from_a_profession_the_build_lacks_is_rejected() {
    let data = data();
    let mut build = mesmer();
    build.skills[0] = Some(SkillId(200)); // a Necromancer skill
    let problems = build.check(&data, SlotKind::Human);
    assert!(
        has(&problems, |p| matches!(
            p,
            LegalityError::SkillNotAvailable { .. }
        )),
        "{problems:?}"
    );

    // With Necromancer as the secondary, the same skill is fine.
    build.secondary = Some(Profession::Necromancer);
    assert!(build.check(&data, SlotKind::Human).is_empty());
}

#[test]
fn a_common_skill_is_allowed_whatever_the_professions() {
    let data = data();
    let mut build = mesmer();
    build.skills[0] = Some(SkillId(300));
    assert!(build.check(&data, SlotKind::Human).is_empty());
}

#[test]
fn a_bar_may_carry_only_one_elite() {
    let data = data();
    let mut build = mesmer();
    build.skills[0] = Some(SkillId(101));
    build.skills[1] = Some(SkillId(102));
    let problems = build.check(&data, SlotKind::Human);
    assert!(has(&problems, |p| matches!(
        p,
        LegalityError::TooManyElites(2)
    )));
}

#[test]
fn a_player_may_carry_three_pve_only_skills_but_not_four() {
    let data = data();
    let mut build = mesmer();
    for index in 0..3 {
        build.skills[index] = Some(SkillId(400 + index as u16));
    }
    assert!(build.check(&data, SlotKind::Human).is_empty());

    build.skills[3] = Some(SkillId(403));
    let problems = build.check(&data, SlotKind::Human);
    assert!(has(&problems, |p| matches!(
        p,
        LegalityError::TooManyPveOnly(4)
    )));
}

#[test]
fn a_hero_may_carry_no_pve_only_skills_at_all() {
    // They are never unlocked for the account, so a hero cannot have one.
    let data = data();
    let mut build = mesmer();
    build.skills[0] = Some(SkillId(400));

    assert!(build.check(&data, SlotKind::Human).is_empty());

    let problems = build.check(&data, SlotKind::Hero);
    assert!(
        has(&problems, |p| matches!(p, LegalityError::PveOnlyOnHero(1))),
        "{problems:?}"
    );
    assert!(!build.check(&data, SlotKind::Henchman).is_empty());
}

#[test]
fn a_skill_may_not_appear_twice() {
    let data = data();
    let mut build = mesmer();
    build.skills[0] = Some(SkillId(100));
    build.skills[1] = Some(SkillId(100));
    let problems = build.check(&data, SlotKind::Human);
    assert!(has(&problems, |p| matches!(
        p,
        LegalityError::DuplicateSkill(SkillId(100))
    )));
}

#[test]
fn a_build_may_not_spend_more_than_two_hundred_points() {
    let data = data();
    let mut build = mesmer();
    build
        .attribute_points
        .insert(Attribute::DominationMagic, 12);
    build.attribute_points.insert(Attribute::FastCasting, 12);
    build.attribute_points.insert(Attribute::IllusionMagic, 12);
    // 97 * 3 = 291.
    let problems = build.check(&data, SlotKind::Human);
    assert!(
        has(&problems, |p| matches!(
            p,
            LegalityError::TooManyPoints { spent: 291 }
        )),
        "{problems:?}"
    );
}

#[test]
fn points_may_not_buy_a_rank_above_twelve() {
    let data = data();
    let mut build = mesmer();
    build
        .attribute_points
        .insert(Attribute::DominationMagic, 13);
    let problems = build.check(&data, SlotKind::Human);
    assert!(has(&problems, |p| matches!(
        p,
        LegalityError::RankTooHigh { rank: 13, .. }
    )));
}

#[test]
fn an_attribute_of_a_profession_the_build_lacks_is_rejected() {
    let data = data();
    let mut build = mesmer();
    build.attribute_points.insert(Attribute::FireMagic, 10);
    let problems = build.check(&data, SlotKind::Human);
    assert!(has(&problems, |p| matches!(
        p,
        LegalityError::AttributeNotAvailable {
            attribute: Attribute::FireMagic,
            ..
        }
    )));
}

#[test]
fn a_secondary_profession_does_not_grant_its_primary_attribute() {
    // The rule most easily missed: a Me/N gets Blood, Death and Curses, but
    // not Soul Reaping.
    let data = data();
    let mut build = mesmer();
    build.secondary = Some(Profession::Necromancer);

    build.attribute_points.insert(Attribute::BloodMagic, 9);
    assert!(build.check(&data, SlotKind::Human).is_empty());

    build.attribute_points.insert(Attribute::SoulReaping, 9);
    let problems = build.check(&data, SlotKind::Human);
    assert!(
        has(&problems, |p| matches!(
            p,
            LegalityError::AttributeNotAvailable {
                attribute: Attribute::SoulReaping,
                ..
            }
        )),
        "{problems:?}"
    );
}

#[test]
fn a_rune_for_a_non_primary_attribute_is_rejected() {
    let data = data();
    let mut build = mesmer();
    build.secondary = Some(Profession::Necromancer);
    build.armor[0].rune = Some("superior-death-magic".parse().unwrap());

    let problems = build.check(&data, SlotKind::Human);
    assert!(
        has(&problems, |p| matches!(
            p,
            LegalityError::RuneNotPrimary {
                attribute: Attribute::DeathMagic,
                ..
            }
        )),
        "{problems:?}"
    );
}

#[test]
fn an_insignia_of_another_profession_is_rejected() {
    let data = data();
    let mut build = mesmer();
    build.armor[0].insignia = Some("tormentors".parse().unwrap());

    let problems = build.check(&data, SlotKind::Human);
    assert!(
        has(&problems, |p| matches!(
            p,
            LegalityError::InsigniaNotPrimary {
                profession: Profession::Necromancer,
                ..
            }
        )),
        "{problems:?}"
    );
}

#[test]
fn headgear_may_only_boost_a_primary_attribute() {
    let data = data();
    let mut build = mesmer();
    build.secondary = Some(Profession::Necromancer);
    build.headgear_attribute = Some(Attribute::BloodMagic);

    let problems = build.check(&data, SlotKind::Human);
    assert!(has(&problems, |p| matches!(
        p,
        LegalityError::HeadgearNotPrimary { .. }
    )));
}

#[test]
fn a_skill_with_no_data_file_is_reported() {
    let data = data();
    let mut build = mesmer();
    build.skills[0] = Some(SkillId(9999));
    let problems = build.check(&data, SlotKind::Human);
    assert!(has(&problems, |p| matches!(
        p,
        LegalityError::UnknownSkill(SkillId(9999))
    )));
}

#[test]
fn every_problem_is_reported_not_just_the_first() {
    let data = data();
    let mut build = mesmer();
    build.secondary = Some(Profession::Mesmer);
    build.attribute_points.insert(Attribute::FireMagic, 13);
    build.skills[0] = Some(SkillId(101));
    build.skills[1] = Some(SkillId(102));

    let problems = build.check(&data, SlotKind::Human);
    assert!(
        problems.len() >= 3,
        "expected several problems, got {problems:?}"
    );
}

#[test]
fn every_legality_message_says_what_is_wrong() {
    let data = data();
    let mut build = mesmer();
    build.secondary = Some(Profession::Mesmer);
    build.attribute_points.insert(Attribute::FireMagic, 13);

    for problem in build.check(&data, SlotKind::Human) {
        let message = problem.to_string();
        assert!(
            message.len() > 20 && message.contains(' '),
            "unhelpful message: {message}"
        );
    }
}

// ------------------------------------------------------------- T1.4.8 foes

fn kournan_seer() -> Foe {
    ron::from_str(
        r#"(
    name: "Kournan Seer",
    wiki: "Kournan Seer",
    affiliation: "kournan",
    species: "Human",
    traits: [Fleshy],
    professions: (Mesmer, None),
    level: (nm: 20, hm: Some(26)),
    attributes: (nm: [(DominationMagic, 15), (InspirationMagic, 14)]),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Kournan_Seer"],
        crawled: "2026-09-22",
        review: NumbersOnly,
    ),
)"#,
    )
    .expect("the seer should parse")
}

#[test]
fn a_foes_level_comes_from_its_own_file() {
    let foe = kournan_seer();
    assert_eq!(foe_level(&foe, false), Some(20));
    assert_eq!(foe_level(&foe, true), Some(26));
}

#[test]
fn a_foe_with_no_hard_mode_level_gives_none_rather_than_a_guess() {
    // The mapping table has overlaps and gaps, so guessing here would be
    // indistinguishable from a researched value.
    let mut foe = kournan_seer();
    foe.level.hm = None;
    assert_eq!(foe_level(&foe, true), None);

    // The mapping is still available to a caller that wants it explicitly.
    let core = core();
    assert_eq!(mapped_hm_level(&core, 20, HmLevelKind::NonBoss), Some(26));
}

#[test]
fn foe_armor_falls_back_to_three_per_level() {
    let core = core();
    let foe = kournan_seer();
    // No armor table on this fixture, so 3 x 20 = 60 for a Mesmer.
    assert_eq!(foe_armor(&foe, &core, DamageType::Fire, false), 60);
}

#[test]
fn a_foes_own_armor_table_beats_the_formula() {
    let core = core();
    let mut foe = kournan_seer();
    foe.armor.default = Some(96);
    foe.armor.per_type = vec![(DamageType::Slashing, 116)];

    assert_eq!(foe_armor(&foe, &core, DamageType::Slashing, false), 116);
    assert_eq!(foe_armor(&foe, &core, DamageType::Fire, false), 96);
}

#[test]
fn hard_mode_does_not_raise_armor_with_level() {
    // The rule that surprises people: a level-26 hard-mode foe keeps its
    // level-20 armor. Only foes *below* 20 in normal mode gain any.
    let core = core();
    let foe = kournan_seer();
    assert_eq!(foe_armor(&foe, &core, DamageType::Fire, true), 60);

    let mut weak = kournan_seer();
    weak.level.nm = 10;
    assert_eq!(foe_armor(&weak, &core, DamageType::Fire, false), 30);
    // In hard mode it is treated as level 20 for armor: 60, not 3 x 26.
    assert_eq!(foe_armor(&weak, &core, DamageType::Fire, true), 60);
}

#[test]
fn a_warrior_foe_gets_its_professions_damage_type_bonus() {
    let core = core();
    let mut foe = kournan_seer();
    foe.professions = (Profession::Warrior, None);

    // 3 x 20 = 60, plus the Warrior's +20 against physical damage.
    assert_eq!(foe_armor(&foe, &core, DamageType::Slashing, false), 80);
    assert_eq!(foe_armor(&foe, &core, DamageType::Fire, false), 60);
}

#[test]
fn foe_energy_is_by_profession_with_an_extra_pip() {
    let core = core();
    let stats = foe_energy(&core, Profession::Mesmer);
    assert_eq!(stats.max, 40);
    // Players get 4 pips; foes get 5.
    assert_eq!(stats.regen_pips, 5);

    assert_eq!(foe_energy(&core, Profession::Warrior).max, 20);
    assert_eq!(foe_energy(&core, Profession::Warrior).regen_pips, 3);
}

#[test]
fn missing_hard_mode_attributes_fall_back_to_a_005() {
    let core = core();
    let foe = kournan_seer(); // no hm attributes

    let (normal, assumed) = foe_attributes(&foe, &core, false);
    assert!(!assumed);
    assert_eq!(
        normal,
        vec![
            (Attribute::DominationMagic, 15),
            (Attribute::InspirationMagic, 14)
        ]
    );

    let (hard, assumed) = foe_attributes(&foe, &core, true);
    assert!(assumed, "the caller must be told A-005 was used");
    // +5 each, capped at 20.
    assert_eq!(
        hard,
        vec![
            (Attribute::DominationMagic, 20),
            (Attribute::InspirationMagic, 19)
        ]
    );
}

#[test]
fn given_hard_mode_attributes_are_used_as_written() {
    let core = core();
    let mut foe = kournan_seer();
    foe.attributes.hm = Some(vec![
        (Attribute::DominationMagic, 20),
        (Attribute::InspirationMagic, 14),
    ]);

    let (hard, assumed) = foe_attributes(&foe, &core, true);
    assert!(
        !assumed,
        "nothing was assumed, so nothing should be flagged"
    );
    assert_eq!(hard[1].1, 14, "Inspiration should not have been raised");
}

#[test]
fn the_a_005_fallback_caps_at_twenty() {
    let core = core();
    let mut foe = kournan_seer();
    foe.attributes.nm = vec![(Attribute::DominationMagic, 18)];

    let (hard, _) = foe_attributes(&foe, &core, true);
    assert_eq!(hard[0].1, 20, "18 + 5 should cap at 20, not reach 23");
}
