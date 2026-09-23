//! Stat modifiers and caps (T3.3.4, T3.6.5, ENG-33, ENG-34).
//!
//! A unit's stats are computed on demand from three sources: permanent
//! modifiers (runes, hard-mode bonuses), gear effects whose conditions are
//! checked at the moment of asking (insignias), and the `while_active`
//! modifiers of the effects on it. **Every cap is applied in one place**,
//! [`combine`], so no skill or effect can bypass one: a later effect that
//! "just adds 40% speed" still ends at +34% unless it is marked as a single
//! skill's own effect that exceeds the cap.
//!
//! Percentage modifiers stack multiplicatively (ENG-34): two 50% block
//! chances give 75%, and two 20% speed boosts give 44%, before the cap.

use gwsim_data::core::ArmorSlot;
use gwsim_data::dsl::{Action, ModCategory, Stat};

/// Where a modifier came from, for attribution and removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSource {
    /// Gear worn all fight.
    Gear,
    /// Hard mode's foe bonuses (`modes.ron`).
    HardMode,
    /// An active effect instance.
    Effect(u32),
    /// A condition.
    Condition,
}

/// One change to one stat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Modifier {
    pub stat: Stat,
    /// For percentage stats, the percentage (−50 is half); for flat stats
    /// (armor, pips, energy, ranks), the amount.
    pub value: f64,
    pub category: ModCategory,
    /// A single skill's own effect may exceed the cap where the wiki says so.
    pub exceeds_cap: bool,
    pub source: ModSource,
}

/// A gear effect: DSL actions evaluated when a stat is asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct GearEffect {
    /// The item's name, for logs.
    pub name: String,
    pub actions: Vec<Action>,
    /// For per-piece insignias, the armor piece it sits on.
    pub piece: Option<ArmorSlot>,
}

/// How a stat's modifiers combine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Combine {
    /// A multiplier `Π(1 + v/100)`, clamped.
    Multiplier { min: f64, max: f64 },
    /// An attack-speed style multiplier on *duration*: "20% faster" makes
    /// each attack 20% shorter, so `Π(1 − v/100)`, clamped.
    Duration { min: f64, max: f64 },
    /// A chance: `1 − Π(1 − p/100)`, as a fraction, capped at 1.
    Chance,
    /// A sum, clamped.
    Sum { min: f64, max: f64 },
}

/// How each stat combines, with its ENG-33 cap.
pub fn combine_rule(stat: Stat) -> Combine {
    match stat {
        // +33% faster attacks at most; −50% slower.
        Stat::AttackSpeed => Combine::Duration {
            min: 0.67,
            max: 1.5,
        },
        Stat::MovementSpeed => Combine::Multiplier {
            min: 0.5,
            max: 1.34,
        },
        // −25% to +150%.
        Stat::ActivationTime => Combine::Multiplier {
            min: 0.75,
            max: 2.5,
        },
        // −50%, no upper cap.
        Stat::Recharge => Combine::Multiplier {
            min: 0.5,
            max: f64::INFINITY,
        },
        Stat::AdrenalineRate => Combine::Multiplier { min: 0.5, max: 2.0 },
        Stat::ProjectileSpeed => Combine::Multiplier { min: 0.5, max: 2.0 },
        // Healing reduction capped at −40%; increases uncapped.
        Stat::HealingReceived => Combine::Multiplier {
            min: 0.6,
            max: f64::INFINITY,
        },
        Stat::DamageDealt | Stat::DamageTaken => Combine::Multiplier {
            min: 0.0,
            max: f64::INFINITY,
        },
        Stat::BlockChance => Combine::Chance,
        Stat::HealthRegeneration | Stat::EnergyRegeneration => Combine::Sum {
            min: -10.0,
            max: 10.0,
        },
        Stat::AttributeRank(_) | Stat::ElementalAttributes => Combine::Sum {
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
        },
        Stat::MaxEnergy | Stat::MaxHealth | Stat::Armor => Combine::Sum {
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
        },
    }
}

/// Combines modifiers for one stat, applying its cap.
///
/// Returns the multiplier (for multiplier and duration stats), the chance
/// (for chance stats) or the sum (for sum stats). A modifier marked
/// `exceeds_cap` is applied after the clamp, so it can take the result past
/// the cap on its own.
pub fn combine<'a>(stat: Stat, modifiers: impl IntoIterator<Item = &'a Modifier>) -> f64 {
    let rule = combine_rule(stat);
    let mut capped = match rule {
        Combine::Multiplier { .. } | Combine::Duration { .. } => 1.0,
        Combine::Chance => 1.0,
        Combine::Sum { .. } => 0.0,
    };
    let mut uncapped = capped;
    for modifier in modifiers {
        if modifier.stat != stat {
            continue;
        }
        let slot = if modifier.exceeds_cap {
            &mut uncapped
        } else {
            &mut capped
        };
        match rule {
            Combine::Multiplier { .. } => *slot *= 1.0 + modifier.value / 100.0,
            Combine::Duration { .. } => *slot *= 1.0 - modifier.value / 100.0,
            Combine::Chance => *slot *= 1.0 - (modifier.value / 100.0).clamp(0.0, 1.0),
            Combine::Sum { .. } => *slot += modifier.value,
        }
    }
    match rule {
        Combine::Multiplier { min, max } | Combine::Duration { min, max } => {
            capped.clamp(min, max) * uncapped
        }
        Combine::Chance => (1.0 - capped * uncapped).clamp(0.0, 1.0),
        Combine::Sum { min, max } => capped.clamp(min, max) + uncapped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modifier(stat: Stat, value: f64) -> Modifier {
        Modifier {
            stat,
            value,
            category: ModCategory::Multiplicative,
            exceeds_cap: false,
            source: ModSource::Gear,
        }
    }

    #[test]
    fn two_fifty_percent_blocks_give_seventy_five() {
        // §17.2 block stacking.
        let chance = combine(
            Stat::BlockChance,
            &[
                modifier(Stat::BlockChance, 50.0),
                modifier(Stat::BlockChance, 50.0),
            ],
        );
        assert!((chance - 0.75).abs() < 1e-12);
    }

    #[test]
    fn flail_and_fall_back_give_eighty_nine_point_one_percent() {
        // §17.2 movement stacking: Flail is −33% speed, "Fall Back!" +33%.
        let speed = combine(
            Stat::MovementSpeed,
            &[
                modifier(Stat::MovementSpeed, -33.0),
                modifier(Stat::MovementSpeed, 33.0),
            ],
        );
        assert!((speed - 0.8911).abs() < 1e-4, "{speed}");
    }

    #[test]
    fn every_cap_holds() {
        let many = |stat, value| combine(stat, &vec![modifier(stat, value); 10]);
        assert_eq!(many(Stat::MovementSpeed, 50.0), 1.34);
        assert_eq!(many(Stat::MovementSpeed, -50.0), 0.5);
        assert_eq!(many(Stat::AttackSpeed, 50.0), 0.67);
        assert_eq!(many(Stat::AttackSpeed, -50.0), 1.5);
        assert_eq!(many(Stat::ActivationTime, -50.0), 0.75);
        assert_eq!(many(Stat::ActivationTime, 100.0), 2.5);
        assert_eq!(many(Stat::Recharge, -50.0), 0.5);
        assert_eq!(many(Stat::HealingReceived, -30.0), 0.6);
        assert_eq!(many(Stat::AdrenalineRate, 100.0), 2.0);
        assert_eq!(many(Stat::HealthRegeneration, 3.0), 10.0);
        assert_eq!(many(Stat::EnergyRegeneration, -3.0), -10.0);
    }

    #[test]
    fn a_single_skill_may_exceed_the_cap() {
        let mut boost = modifier(Stat::MovementSpeed, 50.0);
        boost.exceeds_cap = true;
        assert_eq!(combine(Stat::MovementSpeed, &[boost]), 1.5);
    }
}
