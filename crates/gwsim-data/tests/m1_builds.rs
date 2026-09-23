//! T1.4.10: every M1 party slot's derived stats match the T1.4.1 hand values.
//!
//! Each build is assembled from two sources that were worked out
//! independently: the **attribute points** come from decoding the §20.1
//! template code (T1.3.1), and the **runes, insignias and headgear** come
//! from §20.1's gear column. If the two disagree, the effective ranks will
//! not match T1.4.1's table and these tests fail — which is the point.

use std::path::PathBuf;

use gwsim_data::build::{ArmorPiece, Build};
use gwsim_data::core::{ArmorSlot, Attribute, CoreData, DamageType, Profession};
use gwsim_data::dataset::DataSet;
use gwsim_data::derived::{
    MAX_POINTS, armor_profile, effective_ranks, energy, max_health, points_spent,
};
use gwsim_data::source::DirSource;
use gwsim_data::template::SkillTemplate;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn core() -> CoreData {
    CoreData::load(data_dir().join("core")).expect("data/core should load")
}

fn data() -> DataSet {
    DataSet::load(&DirSource::new(data_dir())).expect("data/ should load")
}

/// One M1 slot, as T1.4.1 §9 records it.
struct Slot {
    label: &'static str,
    /// The published template code the attribute points come from.
    code: &'static str,
    headgear: Attribute,
    /// The rune on each of the five armor pieces, head first.
    runes: [Option<&'static str>; 5],
    insignia: &'static str,
    /// The weapon set, as slugs. A staff is two-handed and has no offhand.
    main_hand: Option<&'static str>,
    offhand: Option<&'static str>,
    /// Effective ranks after runes and headgear.
    expected_ranks: &'static [(Attribute, u8)],
    expected_points: u16,
    expected_health: i32,
    expected_energy: i32,
    expected_pips: u8,
    expected_armor: i16,
}

const SLOTS: &[Slot] = &[
    Slot {
        label: "player, Me/-- Energy Surge",
        code: "OQBTAUBPQaJ4EY6x0BAAAAAAuE",
        headgear: Attribute::DominationMagic,
        // A-033, settled by the owner 2026-09-22 from the current PvX page.
        // Three attribute runes plus Superior Vigor and one Vitae fills all
        // five slots exactly, which is why only one Vitae fits here where
        // the two-attribute heroes carry two.
        runes: [
            Some("superior-domination-magic"),
            Some("minor-fast-casting"),
            Some("minor-inspiration-magic"),
            Some("superior-vigor"),
            Some("vitae"),
        ],
        insignia: "prodigys",
        main_hand: Some("wand"),
        offhand: Some("focus"),
        expected_ranks: &[
            (Attribute::FastCasting, 11),
            (Attribute::DominationMagic, 16),
            (Attribute::InspirationMagic, 9),
        ],
        expected_points: 195,
        expected_health: 465,
        expected_energy: 42,
        expected_pips: 4,
        expected_armor: 60,
    },
    Slot {
        label: "heroes 1 to 3, Domination Mesmer",
        code: "OQBTAWBPsBAkDmemuhAONDAAA",
        headgear: Attribute::DominationMagic,
        runes: [
            Some("superior-domination-magic"),
            Some("major-fast-casting"),
            Some("minor-inspiration-magic"),
            Some("superior-vigor"),
            Some("vitae"),
        ],
        insignia: "prodigys",
        main_hand: Some("wand"),
        offhand: Some("focus"),
        expected_ranks: &[
            (Attribute::FastCasting, 13),
            (Attribute::DominationMagic, 16),
            (Attribute::InspirationMagic, 7),
        ],
        expected_points: 195,
        expected_health: 430,
        expected_energy: 42,
        expected_pips: 4,
        expected_armor: 60,
    },
    Slot {
        label: "hero 4, Minion Master N/P",
        code: "OAljUwGpZS8Y7Y1YVVUBKgbhAAA",
        headgear: Attribute::DeathMagic,
        runes: [
            Some("superior-death-magic"),
            Some("major-soul-reaping"),
            Some("superior-vigor"),
            Some("vitae"),
            Some("vitae"),
        ],
        insignia: "minion-masters",
        main_hand: Some("wand"),
        offhand: Some("focus"),
        expected_ranks: &[
            (Attribute::DeathMagic, 16),
            (Attribute::SoulReaping, 11),
            // Command is a secondary-profession attribute, so it cannot take
            // a rune and is stuck at whatever points buy.
            (Attribute::Command, 9),
        ],
        expected_points: 193,
        expected_health: 440,
        expected_energy: 42,
        expected_pips: 4,
        expected_armor: 60,
    },
    Slot {
        label: "hero 5, Blood is Power N/Rt",
        code: "OAhjQkGZIP3hhmwrqKNncDzqH",
        headgear: Attribute::BloodMagic,
        runes: [
            Some("superior-blood-magic"),
            Some("major-soul-reaping"),
            Some("superior-vigor"),
            Some("vitae"),
            Some("vitae"),
        ],
        insignia: "tormentors",
        main_hand: Some("wand"),
        offhand: Some("focus"),
        expected_ranks: &[
            (Attribute::BloodMagic, 13),
            (Attribute::SoulReaping, 11),
            (Attribute::RestorationMagic, 12),
        ],
        expected_points: 193,
        expected_health: 440,
        expected_energy: 42,
        expected_pips: 4,
        expected_armor: 60,
    },
    Slot {
        label: "hero 6, Signet of Spirits Rt",
        code: "OACjEyiM5MXTvJzEAINncDzxJ",
        headgear: Attribute::ChannelingMagic,
        runes: [
            Some("superior-channeling-magic"),
            Some("major-restoration-magic"),
            Some("minor-spawning-power"),
            Some("superior-vigor"),
            Some("vitae"),
        ],
        insignia: "shamans",
        main_hand: Some("wand"),
        offhand: Some("focus"),
        expected_ranks: &[
            (Attribute::RestorationMagic, 14),
            (Attribute::ChannelingMagic, 16),
            (Attribute::SpawningPower, 4),
        ],
        expected_points: 200,
        expected_health: 430,
        expected_energy: 42,
        expected_pips: 4,
        expected_armor: 60,
    },
    Slot {
        label: "hero 7, Soul Twisting Rt/Mo",
        code: "OACiAyk8gNtePuwJ00ZaNBAA",
        headgear: Attribute::Communing,
        // Two superior runes: -150 health before any Vigor.
        runes: [
            Some("superior-communing"),
            Some("superior-spawning-power"),
            Some("superior-vigor"),
            Some("vitae"),
            Some("vitae"),
        ],
        insignia: "shamans",
        // The only slot without a 40/40 set: a Spawning Power staff. Its
        // +10 energy is two less than a focus's +12. Its two health
        // modifiers are not encoded -- see fallout E5.
        main_hand: Some("staff"),
        offhand: None,
        expected_ranks: &[(Attribute::Communing, 16), (Attribute::SpawningPower, 15)],
        expected_points: 194,
        expected_health: 400,
        expected_energy: 40,
        expected_pips: 4,
        expected_armor: 60,
    },
];

/// Builds a slot from its published code plus its §20.1 gear.
fn assemble(slot: &Slot) -> Build {
    let template =
        SkillTemplate::decode(slot.code).unwrap_or_else(|error| panic!("{}: {error}", slot.label));

    let mut build = Build::new(template.primary);
    build.secondary = template.secondary;
    build.skills = template.skills;
    for (attribute, rank) in &template.attributes {
        build.attribute_points.insert(*attribute, *rank);
    }
    build.headgear_attribute = Some(slot.headgear);

    build.armor = std::array::from_fn(|index| ArmorPiece {
        slot: ArmorSlot::ALL[index],
        insignia: Some(slot.insignia.parse().unwrap()),
        rune: slot.runes[index].map(|name| name.parse().unwrap()),
    });

    build.weapon_set.main = slot.main_hand.map(|name| name.parse().unwrap());
    build.weapon_set.offhand = slot.offhand.map(|name| name.parse().unwrap());

    build
}

#[test]
fn every_slot_has_the_expected_effective_ranks() {
    let data = data();
    for slot in SLOTS {
        let build = assemble(slot);
        let ranks = effective_ranks(&build, &data);

        for (attribute, expected) in slot.expected_ranks {
            assert_eq!(
                ranks.get(attribute),
                Some(expected),
                "{}: {attribute:?} should be rank {expected}, ranks are {ranks:?}",
                slot.label
            );
        }
        assert_eq!(
            ranks.len(),
            slot.expected_ranks.len(),
            "{}: unexpected extra attributes in {ranks:?}",
            slot.label
        );
    }
}

#[test]
fn every_slot_is_within_the_attribute_point_budget() {
    for slot in SLOTS {
        let build = assemble(slot);
        let spent = points_spent(&build);
        assert_eq!(spent, slot.expected_points, "{}", slot.label);
        assert!(
            spent <= MAX_POINTS,
            "{} spends {spent} of {MAX_POINTS}",
            slot.label
        );
    }
}

#[test]
fn every_slot_has_the_expected_health() {
    let core = core();
    let data = data();
    for slot in SLOTS {
        let build = assemble(slot);
        assert_eq!(
            max_health(&build, 20, &data, &core),
            slot.expected_health,
            "{}",
            slot.label
        );
    }
}

#[test]
fn every_slot_has_the_expected_energy_and_regeneration() {
    let core = core();
    let data = data();
    for slot in SLOTS {
        let build = assemble(slot);
        let stats = energy(&build, &data, &core);
        assert_eq!(stats.max, slot.expected_energy, "{} energy", slot.label);
        assert_eq!(
            stats.regen_pips, slot.expected_pips,
            "{} regeneration",
            slot.label
        );
    }
}

#[test]
fn every_slot_has_the_expected_resting_armor() {
    let core = core();
    for slot in SLOTS {
        let build = assemble(slot);
        let profile = armor_profile(&build, &core);
        for (index, piece) in profile.pieces.iter().enumerate() {
            assert_eq!(
                piece.against(DamageType::Fire),
                slot.expected_armor,
                "{} piece {index} vs fire",
                slot.label
            );
            assert_eq!(
                piece.against(DamageType::Slashing),
                slot.expected_armor,
                "{} piece {index} vs slashing",
                slot.label
            );
        }
    }
}

// ----------------------------------------------------- the interesting cases

#[test]
fn the_player_outlives_the_mesmer_heroes_by_one_rune_penalty() {
    // 465 against 430, and the whole of the difference is the heroes' major
    // Fast Casting rune. Player and hero carry the same Superior Vigor, the
    // same single Vitae and the same superior Domination penalty; the heroes
    // run Fast Casting at 11+2 where the player runs 10+1.
    let core = core();
    let data = data();
    let player = max_health(&assemble(&SLOTS[0]), 20, &data, &core);
    let hero = max_health(&assemble(&SLOTS[1]), 20, &data, &core);
    // 480 - 75 (Superior Domination) + 50 (Superior Vigor) + 10 (Vitae).
    assert_eq!(480 - 75 + 50 + 10, player);
    assert_eq!(player, 465);
    assert_eq!(hero, 430);
    assert_eq!(player - hero, 35, "a major rune's health penalty");
}

#[test]
fn every_slot_fills_all_five_rune_slots() {
    // The check that caught the missing Vitae on heroes 4, 5 and 7: nobody
    // in a published build leaves a rune slot empty, so an empty one means
    // the loadout was inferred rather than read.
    for slot in SLOTS {
        assert!(
            slot.runes.iter().all(Option::is_some),
            "{}: every armor slot should carry a rune",
            slot.label
        );
    }
}

#[test]
fn two_superior_runes_cost_a_hundred_and_fifty_health() {
    // Hero 7 runs superior Communing and superior Spawning Power. Both
    // penalties apply even though only one bonus does for each attribute.
    let core = core();
    let data = data();
    let hero7 = max_health(&assemble(&SLOTS[5]), 20, &data, &core);
    assert_eq!(hero7, 400);
    // 480 + 50 (Superior Vigor) + 10 + 10 (two Vitae) - 75 - 75 = 400.
    assert_eq!(480 + 50 + 10 + 10 - 75 - 75, hero7);
}

#[test]
fn a_secondary_profession_attribute_cannot_take_a_rune() {
    // Hero 4's Command is a Paragon attribute on a Necromancer primary, so
    // it stays at the 9 its points bought however many runes are worn.
    let data = data();
    let build = assemble(&SLOTS[2]);
    assert!(!build.can_rune(Attribute::Command));
    assert_eq!(
        effective_ranks(&build, &data).get(&Attribute::Command),
        Some(&9)
    );
}

#[test]
fn the_caster_weapon_set_is_what_makes_the_difference_in_energy() {
    // Seven of the eight slots carry a 40/40 set, whose focus adds 12 to the
    // profession base of 30. The Soul Twisting ritualist is the exception: a
    // two-handed staff adds 10, so it is the one slot at 40.
    let core = core();
    let data = data();
    for slot in SLOTS {
        let expected = if slot.offhand == Some("focus") {
            42
        } else {
            40
        };
        assert_eq!(
            energy(&assemble(slot), &data, &core).max,
            expected,
            "{}",
            slot.label
        );
    }
    // Spelled out: the staff costs the ritualist two energy against a focus.
    assert_eq!(energy(&assemble(&SLOTS[5]), &data, &core).max, 40);
}

#[test]
fn hero_six_spends_exactly_the_whole_budget() {
    let build = assemble(&SLOTS[4]);
    assert_eq!(points_spent(&build), MAX_POINTS);
}

#[test]
fn every_slot_decodes_to_the_profession_its_label_says() {
    let expected = [
        Profession::Mesmer,
        Profession::Mesmer,
        Profession::Necromancer,
        Profession::Necromancer,
        Profession::Ritualist,
        Profession::Ritualist,
    ];
    for (slot, profession) in SLOTS.iter().zip(expected) {
        assert_eq!(assemble(slot).primary, profession, "{}", slot.label);
    }
}

#[test]
fn a_build_round_trips_back_to_its_published_skill_code() {
    // T1.4.9's done criterion: decode, build, and encode again.
    let data = data();
    for slot in SLOTS {
        let build = assemble(slot);
        let (skill, _equipment) = build.to_templates(&data);
        assert_eq!(
            skill.encode(),
            slot.code,
            "{} did not round-trip",
            slot.label
        );
    }
}

/// T4.9.1: the party file's slots in the order of [`SLOTS`] (heroes 1 to 3
/// share one row).
const PARTY_ROWS: [usize; 8] = [0, 1, 1, 1, 2, 3, 4, 5];

#[test]
fn the_party_file_matches_the_hand_values_and_its_benchmarks() {
    let core = core();
    let data = data();
    let party = data
        .party(&"m1-mesmerway".parse().unwrap())
        .expect("the M1 party loads");
    assert_eq!(party.slots.len(), 8);
    for (slot, row) in party.slots.iter().zip(PARTY_ROWS) {
        let hand = &SLOTS[row];
        let ranks = effective_ranks(&slot.build, &data);
        for (attribute, expected) in hand.expected_ranks {
            assert_eq!(ranks.get(attribute), Some(expected), "{}", slot.name);
        }
        assert_eq!(
            max_health(&slot.build, 20, &data, &core),
            hand.expected_health,
            "{}",
            slot.name
        );
        assert_eq!(
            energy(&slot.build, &data, &core).max,
            hand.expected_energy,
            "{}",
            slot.name
        );
    }

    // Every bar is in a referenced benchmark, as its bar code or, where the
    // published code is the bar, as that.
    let codes: Vec<String> = party
        .benchmarks
        .iter()
        .map(|slug| &data.benchmarks[slug].value)
        .flat_map(|b| b.slots.iter())
        .map(|s| s.bar_code.clone().unwrap_or_else(|| s.skill_code.clone()))
        .collect();
    assert_eq!(party.benchmarks.len(), 2);
    for slot in &party.slots {
        let (skill, _) = slot.build.to_templates(&data);
        assert!(
            codes.contains(&skill.encode()),
            "{}'s bar {} is in no benchmark",
            slot.name,
            skill.encode()
        );
    }
}
