//! Generating a human slot's priority plan from its build (T4.6.1, §11.5).
//!
//! Each skill becomes one rule, read from what its encoding does:
//!
//! - an effect the user keeps on itself becomes a **maintain** rule
//!   (Air of Superiority);
//! - a skill that only works on a foe using a skill or casting a spell
//!   becomes an **interrupt** rule on the nearest such foe;
//! - a skill that takes energy away goes on the foe with the **most energy**;
//! - a removal goes on an **ally with that effect**; a heal on the
//!   **lowest-health ally** below a threshold;
//! - a hex that punishes casting goes on a **caster**; a skill that does
//!   more when its target is hexed or enchanted goes on such a target;
//! - an area skill goes on the foe with the **most foes near it**;
//! - anything else goes on the called or current target.
//!
//! Rules are ordered by the skill's own AI priority, then by role:
//! maintenance, interrupts, removal and healing, then damage, in bar order
//! within each. The default action attacks the called target, or the
//! nearest foe. The result is ordinary RON a user can edit (T4.6.2).

use gwsim_data::build::Build;
use gwsim_data::dataset::DataSet;
use gwsim_data::dsl::{Action, Control, EffectKind, Filter, Selector};
use gwsim_data::foe::SkillRef;
use gwsim_data::plan::{DefaultAction, Pick, PlanRule, PlanTarget, PriorityPlan, SelfCondition};
use gwsim_data::skill::{Skill, TargetKind};

use super::profile::SkillProfile;

/// Heal allies below this share of health (T4.6.1).
const HEAL_BELOW_PERCENT: f32 = 75.0;

/// The role order rules are sorted by (T4.6.1 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    SelfBuff,
    Interrupt,
    Support,
    Damage,
}

/// Generates a plan for a build.
pub fn generate(build: &Build, data: &DataSet) -> PriorityPlan {
    let mut plan = PriorityPlan {
        maintain: Vec::new(),
        rules: Vec::new(),
        default: DefaultAction::Attack,
    };
    let mut rules: Vec<(i8, Role, usize, PlanRule)> = Vec::new();
    for (index, id) in build.skills.iter().enumerate() {
        let Some(id) = id else { continue };
        let Some(skill) = data.skill_by_id(*id) else {
            continue;
        };
        let profile = SkillProfile::of(skill);
        let priority = skill
            .encoding
            .as_ref()
            .and_then(|e| e.ai.as_ref())
            .map_or(0, |ai| ai.priority);
        let reference = SkillRef::Id(*id);
        match rule_for(skill, &profile) {
            Generated::Maintain => plan.maintain.push(reference),
            Generated::Rule(role, target, only_if) => rules.push((
                priority,
                role,
                index,
                PlanRule {
                    skill: reference,
                    target,
                    only_if,
                },
            )),
        }
    }
    rules.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    plan.rules = rules.into_iter().map(|(_, _, _, rule)| rule).collect();
    plan
}

enum Generated {
    Maintain,
    Rule(Role, PlanTarget, Vec<SelfCondition>),
}

fn rule_for(skill: &Skill, profile: &SkillProfile) -> Generated {
    let foe = |pick: Pick, filter: Option<Filter>| PlanTarget::Foe { pick, filter };
    let ally = |pick: Pick, filter: Option<Filter>| PlanTarget::Ally { pick, filter };
    let self_targeted = matches!(
        skill.target,
        TargetKind::SelfOnly | TargetKind::None | TargetKind::Location
    );

    // Kept up on the user: maintained, unless it is consumed by use (an
    // echo), which is simply used when ready.
    if profile.self_effect.is_some() && self_targeted {
        return if profile.handler.as_deref() == Some("arcane_echo") {
            Generated::Rule(Role::SelfBuff, PlanTarget::SelfUnit, Vec::new())
        } else {
            Generated::Maintain
        };
    }
    if profile.resurrects {
        return Generated::Rule(Role::Support, ally(Pick::LowestHealth, None), Vec::new());
    }
    if profile.interrupts
        && let Some(requirement) = &profile.requirement
    {
        return Generated::Rule(
            Role::Interrupt,
            foe(Pick::Nearest, Some(requirement.clone())),
            Vec::new(),
        );
    }
    if let Some(kind) = profile.removes {
        let (target, role) = match (skill.target, kind) {
            (TargetKind::Foe, _) => (foe(Pick::Nearest, Some(Filter::Enchanted)), Role::Damage),
            (_, EffectKind::Hex) => (ally(Pick::LowestHealth, Some(Filter::Hexed)), Role::Support),
            (_, _) => (ally(Pick::LowestHealth, None), Role::Support),
        };
        return Generated::Rule(role, target, Vec::new());
    }
    if profile.heals_target && !matches!(skill.target, TargetKind::Foe) {
        return Generated::Rule(
            Role::Support,
            ally(
                Pick::LowestHealth,
                Some(Filter::BelowHealth {
                    percent: HEAL_BELOW_PERCENT,
                }),
            ),
            Vec::new(),
        );
    }
    if matches!(skill.target, TargetKind::Foe) {
        let encoding = skill.encoding.as_ref();
        let effects = encoding.map(|e| e.effects.as_slice()).unwrap_or_default();
        if drains_energy(effects) {
            return Generated::Rule(Role::Damage, foe(Pick::MostEnergy, None), Vec::new());
        }
        if punishes_casting(skill) {
            return Generated::Rule(
                Role::Damage,
                foe(Pick::Nearest, Some(Filter::HoldingCasterWeapon)),
                Vec::new(),
            );
        }
        if let Some(condition) = bonus_condition(effects) {
            return Generated::Rule(
                Role::Damage,
                foe(Pick::Nearest, Some(condition)),
                Vec::new(),
            );
        }
        if let Some(requirement) = &profile.requirement {
            return Generated::Rule(
                Role::Damage,
                foe(Pick::Nearest, Some(requirement.clone())),
                Vec::new(),
            );
        }
        if skill.aoe.is_some() {
            return Generated::Rule(Role::Damage, foe(Pick::MostFoesNearby, None), Vec::new());
        }
        return Generated::Rule(Role::Damage, PlanTarget::CalledOrCurrent, Vec::new());
    }
    if matches!(
        skill.target,
        TargetKind::Ally | TargetKind::OtherAlly | TargetKind::AllyOrSelf
    ) {
        return Generated::Rule(Role::Support, ally(Pick::LowestHealth, None), Vec::new());
    }
    Generated::Rule(Role::SelfBuff, PlanTarget::SelfUnit, Vec::new())
}

/// Whether the skill takes energy from its target (Energy Surge).
fn drains_energy(actions: &[Action]) -> bool {
    actions.iter().any(|a| {
        matches!(
            a,
            Action::LoseEnergy {
                to: Selector::TargetFoe | Selector::Target,
                ..
            } | Action::DrainEnergy { .. }
        )
    })
}

/// Whether the skill is a hex that punishes casting (Mistrust).
fn punishes_casting(skill: &Skill) -> bool {
    skill.encoding.as_ref().is_some_and(|e| {
        e.effect_defs.iter().any(|d| {
            d.kind == EffectKind::Hex
                && d.triggers.iter().any(|t| {
                    matches!(
                        t,
                        Control::Triggered {
                            event: gwsim_data::dsl::Event::OnSpellCast,
                            ..
                        }
                    )
                })
        })
    })
}

/// A condition on the target that makes the skill do more (Unnatural
/// Signet's "if that foe is hexed or enchanted").
fn bonus_condition(actions: &[Action]) -> Option<Filter> {
    actions
        .iter()
        .find_map(|a| match a {
            Action::Control(control) => match control.as_ref() {
                Control::If {
                    condition,
                    of: Some(Selector::TargetFoe | Selector::Target) | None,
                    then,
                    ..
                } if then.iter().any(|t| matches!(t, Action::Damage { .. })) => {
                    Some(condition.clone())
                }
                _ => None,
            },
            _ => None,
        })
        .filter(|_| actions.len() > 1)
}
