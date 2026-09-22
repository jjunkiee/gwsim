//! T1.4.10: the DESIGN §17.2 worked examples assigned to WP1.4.
//!
//! Each test names the wiki page its numbers come from. These are the figures
//! the wiki publishes worked out, so a failure here means gwsim disagrees with
//! the game's own documentation — not that a test needs updating.

use std::path::PathBuf;

use gwsim_data::build::Build;
use gwsim_data::core::{Attribute, CoreData, Profession};
use gwsim_data::dataset::DataSet;
use gwsim_data::derived::{
    MAX_POINTS, attribute_cost, check_triplet, effective_ranks, energy, foe_max_health, max_health,
    points_spent, rank_for_points, scaled,
};
use gwsim_data::source::DirSource;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn core() -> CoreData {
    CoreData::load(data_dir().join("core")).expect("data/core should load")
}

fn data() -> DataSet {
    DataSet::load(&DirSource::new(data_dir())).expect("data/ should load")
}

// ------------------------------------------------------------ skill scaling

/// Wiki: Template:Skill progression.
#[test]
fn skill_values_interpolate_between_rank_zero_and_fifteen() {
    // value(rank) = round(at0 + rank * (at15 - at0) / 15)
    // Healing Breeze is {{gr|4|9}}: 4 at rank 0, 8 at 12, 9 at 15.
    assert_eq!(scaled(4, 9, 0), 4);
    assert_eq!(scaled(4, 9, 12), 8);
    assert_eq!(scaled(4, 9, 15), 9);
}

/// Wiki: Mistrust, via DESIGN §20.4.
#[test]
fn mistrusts_published_values_reproduce() {
    // 10 at rank 0, 66 at 12, 80 at 15.
    assert_eq!(scaled(10, 80, 0), 10);
    assert_eq!(scaled(10, 80, 12), 66);
    assert_eq!(scaled(10, 80, 15), 80);
    assert!(check_triplet(10, 66, 80));
}

#[test]
fn scaling_extrapolates_past_rank_fifteen() {
    // Runes and headgear can push an attribute to 20, and skills keep
    // scaling. round(10 + 20 * 70 / 15) = round(103.33) = 103.
    assert_eq!(scaled(10, 80, 20), 103);
}

#[test]
fn a_flat_value_stays_flat_at_every_rank() {
    for rank in 0..=20u8 {
        assert_eq!(scaled(25, 25, rank), 25, "rank {rank}");
    }
}

#[test]
fn scaling_handles_values_that_fall_with_rank() {
    // Some skills scale downwards, such as a recharge that shortens.
    assert_eq!(scaled(20, 5, 0), 20);
    assert_eq!(scaled(20, 5, 15), 5);
    // round(20 - 12 * 15 / 15) = 8.
    assert_eq!(scaled(20, 5, 12), 8);
}

// ---------------------------------------------------------- attribute cost

/// Wiki: Attribute point.
#[test]
fn the_attribute_cost_table_matches_the_wiki() {
    let expected = [0u16, 1, 3, 6, 10, 15, 21, 28, 37, 48, 61, 77, 97];
    for (rank, total) in expected.iter().enumerate() {
        assert_eq!(attribute_cost(rank as u8), *total, "rank {rank}");
    }
}

#[test]
fn rank_twelve_costs_ninety_seven() {
    // The single most-quoted number in build planning.
    assert_eq!(attribute_cost(12), 97);
}

#[test]
fn points_cannot_buy_past_rank_twelve() {
    // Ranks above 12 come from runes and headgear, so they cost no points.
    assert_eq!(attribute_cost(13), 97);
    assert_eq!(attribute_cost(20), 97);
}

#[test]
fn a_budget_buys_the_rank_the_wiki_says() {
    assert_eq!(rank_for_points(97), 12);
    assert_eq!(rank_for_points(96), 11);
    assert_eq!(rank_for_points(77), 11);
    assert_eq!(rank_for_points(0), 0);
    assert_eq!(rank_for_points(MAX_POINTS), 12);
}

#[test]
fn two_attributes_at_twelve_leave_six_points() {
    // The wiki's own worked line: 200 - 97 - 97 = 6.
    assert_eq!(MAX_POINTS - 2 * attribute_cost(12), 6);
}

// ---------------------------------------------------------------- health

/// Wiki: Health, Level.
#[test]
fn health_at_level_twenty_is_four_hundred_and_eighty() {
    let core = core();
    assert_eq!(core.levels.health_at(20), 480);
}

#[test]
fn a_bare_build_has_the_level_health_and_nothing_more() {
    let core = core();
    let data = data();
    let build = Build::new(Profession::Mesmer);
    assert_eq!(max_health(&build, 20, &data, &core), 480);
}

/// Wiki: Health — a Dervish reaches 635 where everyone else reaches 610.
#[test]
fn only_the_dervish_gains_health_from_armor() {
    let core = core();
    let data = data();
    for profession in Profession::ALL {
        let build = Build::new(profession);
        let expected = if profession == Profession::Dervish {
            505
        } else {
            480
        };
        assert_eq!(
            max_health(&build, 20, &data, &core),
            expected,
            "{profession:?}"
        );
    }
}

// ---------------------------------------------------------------- energy

/// Wiki: Energy — the profession table.
#[test]
fn energy_by_profession_matches_the_wiki() {
    let core = core();
    let data = data();

    let expected = [
        (Profession::Warrior, 20, 2u8),
        (Profession::Ranger, 25, 3),
        (Profession::Monk, 30, 4),
        (Profession::Necromancer, 30, 4),
        (Profession::Mesmer, 30, 4),
        (Profession::Elementalist, 30, 4),
        (Profession::Assassin, 25, 4),
        (Profession::Ritualist, 30, 4),
        (Profession::Paragon, 30, 2),
        (Profession::Dervish, 25, 4),
    ];

    for (profession, max, pips) in expected {
        let stats = energy(&Build::new(profession), &data, &core);
        assert_eq!(stats.max, max, "{profession:?} maximum energy");
        assert_eq!(stats.regen_pips, pips, "{profession:?} regeneration");
    }
}

/// Wiki: Energy — the itemised maximum-Elementalist-energy sum of 136.
///
/// **Part of this sum is not yet computable**, and the test says which. The
/// pieces that depend on an insignia's or an inscription's *effect* need the
/// DSL, which is a placeholder until WP1.5. What is asserted through real
/// code is the part that decides whether the Energy Storage and Attunement
/// logic is right — which is the part most likely to be wrong.
#[test]
fn maximum_elementalist_energy_reaches_one_hundred_and_thirty_six() {
    use gwsim_data::build::ArmorPiece;
    use gwsim_data::core::ArmorSlot;

    let core = core();
    let data = data();

    let mut build = Build::new(Profession::Elementalist);
    // Energy Storage 16: 12 points, +1 headgear, +3 superior rune.
    build.attribute_points.insert(Attribute::EnergyStorage, 12);
    build.headgear_attribute = Some(Attribute::EnergyStorage);

    // Four Attunement runes and one Superior Energy Storage. Four, not five:
    // the fifth slot holds the rune that makes rank 16 possible. A test using
    // five would be 2 too high and would look like a rounding bug.
    build.armor = [
        ArmorPiece {
            slot: ArmorSlot::Head,
            insignia: None,
            rune: Some("superior-energy-storage".parse().unwrap()),
        },
        ArmorPiece {
            slot: ArmorSlot::Chest,
            insignia: None,
            rune: Some("attunement".parse().unwrap()),
        },
        ArmorPiece {
            slot: ArmorSlot::Hands,
            insignia: None,
            rune: Some("attunement".parse().unwrap()),
        },
        ArmorPiece {
            slot: ArmorSlot::Legs,
            insignia: None,
            rune: Some("attunement".parse().unwrap()),
        },
        ArmorPiece {
            slot: ArmorSlot::Feet,
            insignia: None,
            rune: Some("attunement".parse().unwrap()),
        },
    ];
    build.weapon_set.offhand = Some("focus".parse().unwrap());

    let ranks = effective_ranks(&build, &data);
    assert_eq!(
        ranks.get(&Attribute::EnergyStorage),
        Some(&16),
        "12 points + 1 headgear + 3 superior rune should give rank 16"
    );

    let computed = energy(&build, &data, &core).max;

    // 20 base + 10 armor + 48 (Energy Storage 16 x 3) + 8 (4 Attunement)
    // + 12 (focus inherent) = 98.
    assert_eq!(computed, 98, "the modelled part of the wiki's sum");

    // The rest comes from item *effects*, which have no typed form until
    // WP1.5: 8 from five Radiant insignias, 15 from a "Seize the Day" wand,
    // and 15 from a "Live for Today" focus inscription.
    const RADIANT_INSIGNIAS: i32 = 8;
    const SEIZE_THE_DAY_WAND: i32 = 15;
    const LIVE_FOR_TODAY_FOCUS: i32 = 15;

    assert_eq!(
        computed + RADIANT_INSIGNIAS + SEIZE_THE_DAY_WAND + LIVE_FOR_TODAY_FOCUS,
        136,
        "the wiki's itemised maximum"
    );
}

// ------------------------------------------------------------- foe health

/// Wiki: Hard mode, Level. DESIGN §20.2.
#[test]
fn hard_mode_foe_health_matches_the_wiki() {
    let core = core();

    // Normal mode is the level formula alone.
    assert_eq!(foe_max_health(&core, 20, false), 480);
    assert_eq!(foe_max_health(&core, 26, false), 600);

    // Hard mode adds 20 per level above 20. A level-26 foe has 720.
    assert_eq!(foe_max_health(&core, 26, true), 720);
    assert_eq!(foe_max_health(&core, 20, true), 480);
    assert_eq!(foe_max_health(&core, 22, true), 560);
}

#[test]
fn the_hard_mode_bonus_applies_only_above_level_twenty() {
    let core = core();
    for level in 1..=20u8 {
        assert_eq!(
            foe_max_health(&core, level, true),
            foe_max_health(&core, level, false),
            "level {level} should be the same in both modes"
        );
    }
}

// --------------------------------------------------------------- spending

#[test]
fn points_spent_adds_up_the_cost_table() {
    let mut build = Build::new(Profession::Mesmer);
    build
        .attribute_points
        .insert(Attribute::DominationMagic, 12);
    build.attribute_points.insert(Attribute::FastCasting, 10);
    build
        .attribute_points
        .insert(Attribute::InspirationMagic, 8);

    // 97 + 61 + 37 = 195, which is what the player's build spends.
    assert_eq!(points_spent(&build), 195);
    assert!(points_spent(&build) <= MAX_POINTS);
}
