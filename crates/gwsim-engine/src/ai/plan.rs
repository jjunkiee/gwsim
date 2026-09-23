//! `PlanAi`: a human slot playing its priority plan (T3.10.5, §11.5).
//!
//! The plan runs exactly as written (A-029): maintenance rules first, then
//! the rules in order, and the first whose skill is usable on a target it
//! can find wins; otherwise the default action. A human reaction delay
//! (A-012) separates each decision from the end of the last action.

use gwsim_data::plan::{DefaultAction, Pick, PlanTarget, SelfCondition};

use crate::effects::EffectSource;
use crate::pipeline::Order;
use crate::sim::Sim;
use crate::unit::{ENERGY_SCALE, Target, UnitId};

/// A plan with its skill references resolved to fight skills.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResolvedPlan {
    pub maintain: Vec<u16>,
    pub rules: Vec<ResolvedRule>,
    pub default: DefaultAction,
}

/// One rule, resolved.
///
/// It names a skill rather than a slot, because what a slot holds can
/// change mid-fight: Arcane Echo becomes a copy of the next spell, and the
/// copy is used by the copied skill's rule.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRule {
    pub skill: u16,
    pub target: PlanTarget,
    pub only_if: Vec<SelfCondition>,
}

/// The slots holding a skill right now, in bar order.
fn slots_holding(sim: &Sim, unit: UnitId, skill: u16) -> Vec<u8> {
    (0..sim.units[unit.index()].bar.len() as u8)
        .filter(|slot| sim.slot_skill(unit, *slot) == Some(skill))
        .collect()
}

/// The plan's next order for a unit, if any.
pub fn decide(sim: &mut Sim, unit: UnitId, plan: u16) -> Option<Order> {
    let fight = std::sync::Arc::clone(&sim.fight);
    let plan = fight.plans.get(usize::from(plan))?;

    for skill in &plan.maintain {
        let up = sim.units[unit.index()].effects.iter().any(|e| {
            matches!(e.source, EffectSource::Skill { skill: s, .. } | EffectSource::Handler { skill: s } if s == *skill)
        });
        if up {
            continue;
        }
        for slot in slots_holding(sim, unit, *skill) {
            if sim.can_use(unit, slot, Target::Unit(unit)).is_ok() {
                return Some(Order::UseSkill {
                    slot,
                    target: Target::Unit(unit),
                });
            }
        }
    }

    for rule in &plan.rules {
        if !rule.only_if.iter().all(|c| self_condition(sim, unit, *c)) {
            continue;
        }
        for slot in slots_holding(sim, unit, rule.skill) {
            let Some(target) = resolve_target(sim, unit, slot, &rule.target) else {
                continue;
            };
            if sim.can_use(unit, slot, target).is_ok() {
                return Some(Order::UseSkill { slot, target });
            }
        }
    }

    match plan.default {
        DefaultAction::Idle => None,
        DefaultAction::Attack => {
            let target = sim.nearest_hostile(unit, engagement_range(sim))?;
            if sim.units[unit.index()].attack_target == Some(target) {
                None
            } else {
                Some(Order::Attack(target))
            }
        }
    }
}

/// How far a plan looks for targets: the aggro bubble (A-009) plus casting
/// range, so a foe that has engaged anyone in the party can be targeted.
fn engagement_range(sim: &Sim) -> f32 {
    sim.fight.tunables.aggro_range
        + sim
            .fight
            .core
            .gwinches(gwsim_data::core::RangeBand::Casting)
}

fn self_condition(sim: &Sim, unit: UnitId, condition: SelfCondition) -> bool {
    let u = &sim.units[unit.index()];
    match condition {
        SelfCondition::EnergyAtLeast(points) => u.energy_points() >= i32::from(points),
        SelfCondition::EnergyBelowPercent(percent) => {
            let max = sim.max_energy(unit).max(1);
            f64::from(u.energy) / f64::from(max * ENERGY_SCALE) * 100.0 < f64::from(percent)
        }
        SelfCondition::HealthBelowPercent(percent) => {
            sim.health_fraction(unit) * 100.0 < f64::from(percent)
        }
    }
}

/// Finds the rule's target among living units.
fn resolve_target(sim: &Sim, unit: UnitId, slot: u8, target: &PlanTarget) -> Option<Target> {
    let me = &sim.units[unit.index()];
    let range = engagement_range(sim);
    match target {
        PlanTarget::SelfUnit => Some(Target::Unit(unit)),
        PlanTarget::CalledOrCurrent => me
            .attack_target
            .filter(|t| sim.units[t.index()].alive())
            .or_else(|| sim.nearest_hostile(unit, range))
            .map(Target::Unit),
        PlanTarget::Foe { pick, filter } | PlanTarget::Ally { pick, filter } => {
            let foes = matches!(target, PlanTarget::Foe { .. });
            let skill = sim.slot_skill(unit, slot);
            let limit = skill
                .and_then(|s| sim.skill_range(unit, s))
                .map_or(range, |r| r.max(range));
            let candidates: Vec<UnitId> = sim
                .units
                .iter()
                .filter(|u| u.alive() && (u.team != me.team) == foes && u.pos.within(me.pos, limit))
                .filter(|u| {
                    filter
                        .as_ref()
                        .is_none_or(|f| sim.filter_passes(f, u.id, unit, None))
                })
                .map(|u| u.id)
                .collect();
            pick_one(sim, unit, &candidates, *pick).map(Target::Unit)
        }
    }
}

/// Picks one candidate by a rule, lowest id first on a tie.
fn pick_one(sim: &Sim, unit: UnitId, candidates: &[UnitId], pick: Pick) -> Option<UnitId> {
    let me = sim.units[unit.index()].pos;
    let score = |id: &UnitId| -> f64 {
        let u = &sim.units[id.index()];
        match pick {
            Pick::Nearest => f64::from(u.pos.distance_squared(me)),
            Pick::MostEnergy => -f64::from(u.energy),
            Pick::LowestHealth => sim.health_fraction(*id),
            Pick::HighestHealth => -sim.health_fraction(*id),
            Pick::MostFoesNearby => {
                let nearby = sim.fight.core.gwinches(gwsim_data::core::RangeBand::Nearby);
                -(sim
                    .units
                    .iter()
                    .filter(|o| o.alive() && o.team == u.team && o.pos.within(u.pos, nearby))
                    .count() as f64)
            }
        }
    };
    candidates
        .iter()
        .copied()
        .min_by(|a, b| score(a).total_cmp(&score(b)).then(a.cmp(b)))
}
