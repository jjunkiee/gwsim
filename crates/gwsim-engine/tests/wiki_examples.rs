//! The wiki's worked examples that the engine owns (DESIGN §17.2, WP3.5).
//!
//! The data-side examples (scaling, attribute costs, health, energy) are in
//! `gwsim-data/tests/wiki_examples.rs`.

use gwsim_data::dsl::{ModCategory, Stat};
use gwsim_engine::combat;
use gwsim_engine::stats::{ModSource, Modifier, combine};

fn modifier(stat: Stat, value: f64) -> Modifier {
    Modifier {
        stat,
        value,
        category: ModCategory::Bonus,
        exceeds_cap: false,
        source: ModSource::Gear,
    }
}

fn close(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= tolerance
}

// ------------------------------------------------------------------- armor

#[test]
fn armor_core_106_with_a_net_minus_25_is_81() {
    // Armor calculation: core 106, bonuses netting −25.
    assert_eq!(combat::armor_level(106.0, &[-25.0], 0.0, 0.0), 81.0);
}

#[test]
fn armor_penetration_of_25_percent_leaves_61_within_one() {
    // The wiki's 61; flooring gives 60 (A-023 allows ±1).
    let armor = combat::armor_level(106.0, &[-25.0], 0.25, 0.0);
    assert!(close(armor, 61.0, 1.0), "{armor}");
}

#[test]
fn special_armor_adds_after_penetration() {
    let armor = combat::armor_level(106.0, &[-25.0], 0.25, 24.0);
    assert!(close(armor, 85.0, 1.0), "{armor}");
}

#[test]
fn positive_bonuses_of_26_or_more_give_the_largest_or_25() {
    // A-031: the documented bug. Reductions are ignored once the positive
    // bonuses reach 26.
    assert_eq!(
        combat::armor_level(60.0, &[15.0, 15.0, -20.0], 0.0, 0.0),
        85.0
    );
    assert_eq!(combat::armor_level(60.0, &[40.0], 0.0, 0.0), 100.0);
}

#[test]
fn reductions_stop_at_60_or_core() {
    assert_eq!(combat::armor_level(80.0, &[-40.0], 0.0, 0.0), 60.0);
    assert_eq!(combat::armor_level(50.0, &[-40.0], 0.0, 0.0), 50.0);
}

// ------------------------------------------------------------------ damage

#[test]
fn skill_damage_at_level_15_is_77_percent() {
    assert!(close(combat::skill_level_factor(15), 0.771, 0.0005));
}

#[test]
fn a_level_30_caster_deals_117_7_from_70_against_60() {
    let damage = combat::damage_packet(70.0, combat::skill_strike_level(30), 60.0);
    assert!(close(damage, 117.7, 0.05), "{damage}");
}

#[test]
fn weapon_rank_8_deals_70_7_percent_of_rank_12() {
    let rank12 = combat::strike_level(12, 20);
    let ratio = combat::damage_packet(1.0, combat::strike_level(8, 20), rank12);
    assert!(close(ratio, 0.707, 0.0005), "{ratio}");
}

#[test]
fn weapon_rank_16_deals_114_9_percent_of_rank_12() {
    let rank12 = combat::strike_level(12, 20);
    let ratio = combat::damage_packet(1.0, combat::strike_level(16, 20), rank12);
    assert!(close(ratio, 1.149, 0.0005), "{ratio}");
}

// ---------------------------------------------------------------- stacking

#[test]
fn two_50_percent_blocks_make_75() {
    let blocks = [
        modifier(Stat::BlockChance, 50.0),
        modifier(Stat::BlockChance, 50.0),
    ];
    assert!(close(combine(Stat::BlockChance, &blocks), 0.75, 1e-12));
}

#[test]
fn flail_with_fall_back_moves_at_89_1_percent() {
    let speed = [
        modifier(Stat::MovementSpeed, -33.0),
        modifier(Stat::MovementSpeed, 33.0),
    ];
    assert!(close(combine(Stat::MovementSpeed, &speed), 0.891, 0.0005));
}

#[test]
fn movement_speed_is_capped_at_plus_34_percent() {
    let speed = [
        modifier(Stat::MovementSpeed, 25.0),
        modifier(Stat::MovementSpeed, 25.0),
    ];
    assert!(close(combine(Stat::MovementSpeed, &speed), 1.34, 1e-12));
}

// -------------------------------------------------------------- conditions

#[test]
fn an_8_second_blind_with_two_20_percent_reductions_lasts_4() {
    assert_eq!(combat::reduced_condition_duration(8000, &[0.2, 0.2]), 4000);
}

// ------------------------------------------------------------ regeneration

#[test]
fn natural_regeneration_starts_at_5_seconds_and_caps_at_7_pips() {
    assert_eq!(combat::natural_regeneration_pips(4999), 0);
    assert_eq!(combat::natural_regeneration_pips(5000), 1);
    assert_eq!(combat::natural_regeneration_pips(7000), 2);
    assert_eq!(combat::natural_regeneration_pips(9000), 3);
    assert_eq!(combat::natural_regeneration_pips(17_000), 7);
    assert_eq!(combat::natural_regeneration_pips(60_000), 7);
}

// ------------------------------------------------------------ fast casting

#[test]
fn fast_casting_halves_activation_at_rank_15() {
    assert!(close(combat::fast_casting_activation(15), 0.5, 1e-12));
    assert!(close(combat::fast_casting_activation(11), 0.6015, 0.00005));
}

#[test]
fn pve_fast_casting_cuts_mesmer_spell_recharge_3_percent_a_rank() {
    assert!(close(combat::fast_casting_recharge(11), 0.67, 1e-12));
    assert!(close(combat::fast_casting_recharge(0), 1.0, 1e-12));
}

// ---------------------------------------------------------------- critical

#[test]
fn a_level_20_attacker_crits_more_often_than_its_mastery_rank() {
    // Critical hit: "if the attacker is level 20, base crit rate is always
    // higher than the weapon's mastery attribute".
    for rank in 0..=16u8 {
        for defender in 1..=30u8 {
            let chance = combat::critical_chance(20, rank, defender);
            assert!(
                chance > f64::from(rank) / 100.0,
                "rank {rank} vs level {defender}: {chance}"
            );
        }
    }
}

#[test]
fn a_critical_hit_multiplies_damage_by_root_two() {
    // Critical hit: "[maximum damage] * 1.414", from 20 points of armor.
    let normal = combat::damage_packet(100.0, 60.0, 60.0);
    let critical = combat::damage_packet(100.0, 80.0, 60.0);
    assert!(close(critical / normal, 1.414, 0.0005));
}
