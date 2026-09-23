//! The wiki's damage, armor, critical-hit and regeneration formulas (WP3.5).
//!
//! Each formula is implemented once, named after the wiki concept, and
//! tested against the wiki's worked examples (ENG-3, §17.2). Nothing here
//! touches a fight: these are pure functions, and [`crate::sim`] applies
//! their results.

use gwsim_data::core::ArmorSlot;

/// The strike level of a martial weapon attack (Damage calculation).
///
/// `5 × rank` up to the threshold `(level + 4) / 2`, then `+2` per rank above
/// it. At level 20 the threshold is rank 12, which gives strike level 60.
pub fn strike_level(weapon_rank: u8, level: u8) -> f64 {
    let threshold = (f64::from(level) + 4.0) / 2.0;
    let rank = f64::from(weapon_rank);
    if rank <= threshold {
        5.0 * rank
    } else {
        5.0 * threshold + 2.0 * (rank - threshold)
    }
}

/// The strike level of skills, wands and staves: three times the level.
pub fn skill_strike_level(level: u8) -> f64 {
    3.0 * f64::from(level)
}

/// One armor-respecting damage packet:
/// `base × 2^((strike level − armor level) / 40)`.
pub fn damage_packet(base: f64, strike_level: f64, armor_level: f64) -> f64 {
    base * 2f64.powf((strike_level - armor_level) / 40.0)
}

/// Skill damage scaled by the caster's level against a 60-armor target:
/// `× 2^((3 × level − 60) / 40)`.
pub fn skill_level_factor(level: u8) -> f64 {
    2f64.powf((skill_strike_level(level) - 60.0) / 40.0)
}

/// Rounds a damage or healing amount to whole points, half away from zero.
pub fn round_points(amount: f64) -> i32 {
    amount.round() as i32
}

/// The armor calculation's four steps (Armor calculation).
///
/// 1. core armor;
/// 2. the net bonus: at 26 or more, add 25 or the largest single bonus if
///    that is bigger (and, per the documented bug A-031, reductions are
///    ignored); at 25 or less, add it; if negative, it lowers armor only down
///    to 60, or to core armor if core is below 60;
/// 3. armor penetration, `× (1 − AP)`, floored (A-023);
/// 4. special armor, uncapped.
pub fn armor_level(core: f64, bonuses: &[f64], penetration: f64, special: f64) -> f64 {
    let net: f64 = bonuses.iter().sum();
    let largest = bonuses.iter().copied().fold(0.0f64, f64::max);
    let positive: f64 = bonuses.iter().filter(|b| **b > 0.0).sum();

    let with_bonus = if positive >= 26.0 {
        // At 26 or more the bonus is capped at 25 (or the largest single
        // bonus), and — the documented bug A-031 — any reductions among the
        // bonuses are ignored.
        core + 25f64.max(largest)
    } else if net >= 0.0 {
        core + net
    } else {
        // A net penalty lowers armor only down to 60, or to core armor when
        // core is below 60.
        (core + net).max(core.min(60.0))
    };

    let penetrated = (with_bonus * (1.0 - penetration.clamp(0.0, 1.0))).floor();
    penetrated + special
}

/// The chance of a critical hit (Critical hit, from the formula the wiki
/// attributes to Isaiah Cartwright).
///
/// `crit = 0.05 × 2^((8·Lₐ + 4·WS + 6·min(WS, (Lₐ+4)/2) − 15·L_d − 100) / 40)
/// × (1 − WS·0.01) + WS·0.01`, where `WS` is the weapon mastery rank (0 for
/// wands and staves).
pub fn critical_chance(attacker_level: u8, weapon_rank: u8, defender_level: u8) -> f64 {
    let la = f64::from(attacker_level);
    let ws = f64::from(weapon_rank);
    let ld = f64::from(defender_level);
    let threshold = (la + 4.0) / 2.0;
    let exponent = (8.0 * la + 4.0 * ws + 6.0 * ws.min(threshold) - 15.0 * ld - 100.0) / 40.0;
    (0.05 * 2f64.powf(exponent) * (1.0 - ws * 0.01) + ws * 0.01).clamp(0.0, 1.0)
}

/// The normal and ranged hit-location column (A-014), in armor-slot order:
/// head 12.5%, chest 37.5%, hands 12.5%, legs 25%, feet 12.5%.
pub const HIT_LOCATION_WEIGHTS: [f64; 5] = [0.125, 0.375, 0.125, 0.25, 0.125];

/// The armor piece a roll in `[0, 1)` lands on.
pub fn hit_location(roll: f64) -> ArmorSlot {
    let mut remaining = roll;
    for (slot, weight) in ArmorSlot::ALL.into_iter().zip(HIT_LOCATION_WEIGHTS) {
        if remaining < weight {
            return slot;
        }
        remaining -= weight;
    }
    ArmorSlot::Feet
}

/// Natural regeneration pips after a time out of combat (Health
/// regeneration): +1 after 5 s, then +1 more every 2 s, up to +7.
pub fn natural_regeneration_pips(out_of_combat_ms: u32) -> i32 {
    if out_of_combat_ms < 5000 {
        return 0;
    }
    (1 + ((out_of_combat_ms - 5000) / 2000) as i32).min(7)
}

/// The Fast Casting activation multiplier, `0.5^(rank / 15)`.
pub fn fast_casting_activation(rank: u8) -> f64 {
    0.5f64.powf(f64::from(rank) / 15.0)
}

/// The PvE Fast Casting recharge multiplier for Mesmer spells, `1 − 3%·rank`.
pub fn fast_casting_recharge(rank: u8) -> f64 {
    (1.0 - 0.03 * f64::from(rank)).max(0.0)
}

/// A condition's duration after reductions, each applied and rounded
/// separately (Condition): an 8 s Blind with two 20% reductions loses 2 s
/// to each (1.6 rounds up) and lasts 4 s.
pub fn reduced_condition_duration(duration_ms: u32, reductions: &[f64]) -> u32 {
    let mut remaining = i64::from(duration_ms);
    for reduction in reductions {
        let lost_s = (f64::from(duration_ms) / 1000.0 * reduction).round();
        remaining -= (lost_s * 1000.0) as i64;
    }
    remaining.max(0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_strike_level_follows_the_threshold() {
        assert_eq!(strike_level(12, 20), 60.0);
        assert_eq!(strike_level(8, 20), 40.0);
        assert_eq!(strike_level(16, 20), 68.0);
    }

    #[test]
    fn a_crit_at_level_twenty_is_rare_for_wands() {
        let chance = critical_chance(20, 0, 20);
        assert!(chance < 0.05, "{chance}");
        // About 12% at rank 12 against a similar level, per the wiki.
        let martial = critical_chance(20, 12, 20);
        assert!((0.10..0.16).contains(&martial), "{martial}");
    }

    #[test]
    fn natural_regeneration_steps_up_to_seven() {
        assert_eq!(natural_regeneration_pips(4999), 0);
        assert_eq!(natural_regeneration_pips(5000), 1);
        assert_eq!(natural_regeneration_pips(6999), 1);
        assert_eq!(natural_regeneration_pips(7000), 2);
        assert_eq!(natural_regeneration_pips(17_000), 7);
        assert_eq!(natural_regeneration_pips(60_000), 7);
    }

    #[test]
    fn hit_locations_follow_the_table() {
        assert_eq!(hit_location(0.0), ArmorSlot::Head);
        assert_eq!(hit_location(0.2), ArmorSlot::Chest);
        assert_eq!(hit_location(0.55), ArmorSlot::Hands);
        assert_eq!(hit_location(0.7), ArmorSlot::Legs);
        assert_eq!(hit_location(0.95), ArmorSlot::Feet);
    }
}
