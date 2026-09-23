//! Tactics plans: how the party sets up for a fight (§11.6, WP4.7).
//!
//! A tactics plan is generated for each situation from the party's builds,
//! and the user may override any part of it in the situation or party file.
//! There are no scripted mid-fight actions (Q29, D14): everything here is
//! decided before the fight and holds throughout.

use serde::{Deserialize, Serialize};

use crate::foe::SkillRef;
use crate::ids::Slug;
use crate::skill::RoleTag;

/// The whole plan.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TacticsPlan {
    /// Where each slot stands relative to the leader, in gwinches: `x` to the
    /// side, `y` forward (negative is behind).
    #[serde(default)]
    pub formation: Vec<FormationPoint>,
    /// Each hero's combat mode.
    #[serde(default)]
    pub hero_modes: Vec<SlotMode>,
    /// Targets the party calls, first match first.
    #[serde(default)]
    pub called_targets: Vec<TargetRule>,
    /// Targets particular slots are locked onto.
    #[serde(default)]
    pub locked_targets: Vec<SlotTarget>,
    /// Skills cast in order before the fight, spending real time and energy.
    #[serde(default)]
    pub pre_fight: Vec<PreCast>,
    /// Skills heroes never use on their own.
    #[serde(default)]
    pub disabled_hero_skills: Vec<SlotSkills>,
    /// Which foe group to engage first, by index, where there are several.
    #[serde(default)]
    pub engage_order: Vec<u16>,
    /// Whether heroes stand apart, against foes with area damage.
    #[serde(default)]
    pub spread_against_aoe: bool,
}

/// A slot's place in the formation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormationPoint {
    pub slot: String,
    pub x: f32,
    pub y: f32,
}

/// A hero's mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotMode {
    pub slot: String,
    pub mode: HeroModeName,
}

/// A hero's combat mode (Hero: Hero Control panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum HeroModeName {
    #[default]
    Fight,
    Guard,
    AvoidCombat,
}

/// A slot and a target it is locked onto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotTarget {
    pub slot: String,
    pub target: TargetRule,
}

/// A slot and some of its skills.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotSkills {
    pub slot: String,
    pub skills: Vec<SkillRef>,
}

/// One pre-fight cast.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreCast {
    pub slot: String,
    pub skill: SkillRef,
    #[serde(default)]
    pub target: TargetRule,
}

/// A way of naming a target before the fight.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum TargetRule {
    /// The caster itself, or no target.
    #[default]
    Caster,
    /// A foe of this kind, by foe file slug.
    Foe(Slug),
    /// A foe whose bar has a skill with this role (a healer).
    FoeWithRole(RoleTag),
    /// The foe nearest the caster.
    Nearest,
    /// A party member, by slot name.
    Slot(String),
}

/// A user's overrides: every field is optional, and each one given is kept
/// while the rest is regenerated (T4.7.5).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TacticsOverrides {
    #[serde(default)]
    pub formation: Option<Vec<FormationPoint>>,
    #[serde(default)]
    pub hero_modes: Option<Vec<SlotMode>>,
    #[serde(default)]
    pub called_targets: Option<Vec<TargetRule>>,
    #[serde(default)]
    pub locked_targets: Option<Vec<SlotTarget>>,
    #[serde(default)]
    pub pre_fight: Option<Vec<PreCast>>,
    #[serde(default)]
    pub disabled_hero_skills: Option<Vec<SlotSkills>>,
    #[serde(default)]
    pub engage_order: Option<Vec<u16>>,
    #[serde(default)]
    pub spread_against_aoe: Option<bool>,
}

impl TacticsPlan {
    /// This plan with a user's overrides applied: each field the user gave
    /// replaces the generated one, whole (T4.7.5). Overrides from the
    /// situation are applied after the party's, so they win.
    pub fn merged(mut self, overrides: &TacticsOverrides) -> TacticsPlan {
        if let Some(v) = &overrides.formation {
            self.formation = v.clone();
        }
        if let Some(v) = &overrides.hero_modes {
            // Per slot: an override for one hero leaves the others alone.
            for mode in v {
                self.hero_modes.retain(|m| m.slot != mode.slot);
                self.hero_modes.push(mode.clone());
            }
        }
        if let Some(v) = &overrides.called_targets {
            self.called_targets = v.clone();
        }
        if let Some(v) = &overrides.locked_targets {
            self.locked_targets = v.clone();
        }
        if let Some(v) = &overrides.pre_fight {
            self.pre_fight = v.clone();
        }
        if let Some(v) = &overrides.disabled_hero_skills {
            self.disabled_hero_skills = v.clone();
        }
        if let Some(v) = &overrides.engage_order {
            self.engage_order = v.clone();
        }
        if let Some(v) = overrides.spread_against_aoe {
            self.spread_against_aoe = v;
        }
        self
    }

    /// A hero's mode in this plan.
    pub fn mode_of(&self, slot: &str) -> HeroModeName {
        self.hero_modes
            .iter()
            .find(|m| m.slot == slot)
            .map(|m| m.mode)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plan_round_trips_in_ron() {
        let plan = TacticsPlan {
            formation: vec![FormationPoint {
                slot: "hero 7".into(),
                x: 100.0,
                y: -300.0,
            }],
            hero_modes: vec![SlotMode {
                slot: "hero 7".into(),
                mode: HeroModeName::Guard,
            }],
            called_targets: vec![TargetRule::FoeWithRole(RoleTag::Healing)],
            pre_fight: vec![PreCast {
                slot: "hero 7".into(),
                skill: SkillRef::Slug("shelter".parse().unwrap()),
                target: TargetRule::Caster,
            }],
            spread_against_aoe: true,
            ..TacticsPlan::default()
        };
        let text = ron::to_string(&plan).unwrap();
        let back: TacticsPlan = ron::from_str(&text).unwrap();
        assert_eq!(back, plan);
    }

    #[test]
    fn an_override_replaces_only_its_field() {
        let generated = TacticsPlan {
            hero_modes: vec![
                SlotMode {
                    slot: "hero 6".into(),
                    mode: HeroModeName::Guard,
                },
                SlotMode {
                    slot: "hero 7".into(),
                    mode: HeroModeName::Guard,
                },
            ],
            spread_against_aoe: true,
            ..TacticsPlan::default()
        };
        let overrides = TacticsOverrides {
            hero_modes: Some(vec![SlotMode {
                slot: "hero 7".into(),
                mode: HeroModeName::Fight,
            }]),
            ..TacticsOverrides::default()
        };
        let merged = generated.merged(&overrides);
        assert_eq!(merged.mode_of("hero 7"), HeroModeName::Fight);
        assert_eq!(merged.mode_of("hero 6"), HeroModeName::Guard);
        assert!(merged.spread_against_aoe);
    }
}
