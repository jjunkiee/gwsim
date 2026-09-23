//! A human slot's priority plan (§11.5, T3.10.5).
//!
//! A plan is an ordered list of rules — "use this skill on that target when
//! these hold" — plus effects to keep up and a default action. The engine's
//! `PlanAi` runs it exactly as written, with a human reaction delay (A-012)
//! and none of the hero AI's quirks (A-029).
//!
//! Plans live with the party, in each slot's `plan` field (T4.6.2), so a
//! user's edits travel with the build they were written for.

use serde::{Deserialize, Serialize};

use crate::dsl::Filter;
use crate::foe::SkillRef;

/// A priority plan.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorityPlan {
    /// Self-effects to keep up, checked before the rules.
    #[serde(default)]
    pub maintain: Vec<SkillRef>,
    /// Rules in priority order; the first usable one wins.
    #[serde(default)]
    pub rules: Vec<PlanRule>,
    /// What to do when no rule applies.
    #[serde(default)]
    pub default: DefaultAction,
}

/// One rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanRule {
    pub skill: SkillRef,
    #[serde(default)]
    pub target: PlanTarget,
    /// Conditions on the user as well as the target.
    #[serde(default)]
    pub only_if: Vec<SelfCondition>,
}

/// Whom a rule aims at.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum PlanTarget {
    /// The called target, else the current attack target, else the nearest
    /// foe.
    #[default]
    CalledOrCurrent,
    /// The user.
    #[serde(rename = "Self")]
    SelfUnit,
    /// A foe, picked by a rule among those passing a filter.
    Foe {
        pick: Pick,
        #[serde(default)]
        filter: Option<Filter>,
    },
    /// An ally (the user included), picked the same way.
    Ally {
        pick: Pick,
        #[serde(default)]
        filter: Option<Filter>,
    },
}

/// How to choose among several valid targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pick {
    Nearest,
    MostEnergy,
    LowestHealth,
    HighestHealth,
    /// The one with the most of its allies near it: the best area target.
    MostFoesNearby,
}

/// A condition on the user.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SelfCondition {
    EnergyAtLeast(u16),
    EnergyBelowPercent(f32),
    HealthBelowPercent(f32),
}

/// What a plan does when no rule applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DefaultAction {
    /// Attack the called target, else the nearest foe.
    #[default]
    Attack,
    /// Stand still.
    Idle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plan_round_trips() {
        let text = r#"(
            maintain: [Slug("air-of-superiority")],
            rules: [
                (skill: Slug("energy-surge"), target: Foe(pick: MostEnergy)),
                (skill: Slug("unnatural-signet"), target: Foe(pick: Nearest, filter: Some(Any([Hexed, Enchanted])))),
                (skill: Slug("power-drain"), target: Foe(pick: Nearest, filter: Some(CastingSpell)), only_if: [EnergyBelowPercent(50.0)]),
            ],
        )"#;
        let plan: PriorityPlan = ron::from_str(text).expect("should parse");
        assert_eq!(plan.rules.len(), 3);
        assert_eq!(plan.default, DefaultAction::Attack);
        let written = ron::to_string(&plan).unwrap();
        let back: PriorityPlan = ron::from_str(&written).unwrap();
        assert_eq!(back, plan);
    }
}
