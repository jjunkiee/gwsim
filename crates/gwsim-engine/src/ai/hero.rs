//! `HeroAi`: heroes act as the wiki documents, quirks included (WP4.5,
//! AI-H1 to AI-H9).
//!
//! - **Targeting (AI-H1):** the called target, else the target the player is
//!   attacking, else — in combat — the foe with the lowest armor, then the
//!   lowest health. Melee heroes stick to their previous target (the
//!   2026-06-24 update).
//! - **Modes (AI-H2):** Fight goes after targets; Guard holds its position
//!   and fights only what reaches it; Avoid Combat never attacks and uses
//!   only indirect skills.
//! - **Quirks (AI-H3 to AI-H5):** heroes do not coordinate — two may cleanse
//!   the same hex, since each reads the party alone; they react to a foe's
//!   cast at once (A-011: no delay for interrupts); they never reapply an
//!   effect that is up.
//! - **Skill-type rules (AI-H6):** batteries only on casters at half energy;
//!   shouts only in combat, speed boosts when moving; passive spirits only in
//!   combat. These live in the shared evaluator ([`super::skills`]).
//! - **No pre-casting (AI-H8):** out of combat a hero casts nothing but
//!   upkeep; pre-fight casting comes only from the tactics plan (WP4.7).
//! - Out of combat, a hero follows the player at its formation offset.

use crate::ai::skills::Situation;
use crate::pipeline::Order;
use crate::sim::Sim;
use gwsim_data::build::SlotKind;

use crate::unit::{HeroMode, Team, UnitId, UnitKind};

/// How far a hero lets its formation point drift before walking to it.
const FOLLOW_SLACK: f32 = 150.0;

/// The hero's next order, if any.
pub fn decide(sim: &mut Sim, unit: UnitId) -> Option<Order> {
    let in_combat = sim.party_engaged(unit);
    let focus = sim.hero_focus(unit, in_combat);
    let mode = sim.units[unit.index()].hero_mode;
    let situation = Situation {
        focus,
        in_combat,
        no_offence: mode == HeroMode::AvoidCombat,
        locked: focus.is_some(),
        interrupts_only: false,
    };
    if let Some((slot, target)) = sim.best_skill(unit, &situation) {
        let within_reach = match (mode, target.unit()) {
            (HeroMode::Guard, Some(t)) if t != unit => {
                let skill = sim.slot_skill(unit, slot)?;
                let range = sim.skill_range(unit, skill).unwrap_or(f32::INFINITY);
                sim.units[unit.index()]
                    .pos
                    .within(sim.units[t.index()].pos, range)
            }
            _ => true,
        };
        if within_reach {
            return Some(Order::UseSkill { slot, target });
        }
    }
    if in_combat
        && mode != HeroMode::AvoidCombat
        && let Some(target) = focus
        && sim.units[unit.index()].attack_target != Some(target)
        && sim.units[unit.index()].weapon.is_some()
    {
        let reach = sim.units[unit.index()]
            .weapon
            .as_ref()
            .map_or(0.0, |w| w.range);
        let close = sim.units[unit.index()]
            .pos
            .within(sim.units[target.index()].pos, reach);
        if mode == HeroMode::Fight || close {
            return Some(Order::Attack(target));
        }
    }
    if in_combat {
        return None;
    }
    sim.follow_leader(unit)
}

/// An interrupt the hero can start at once, inside its reaction delay: heroes
/// react to a foe's cast without one (A-011, Hero behavior).
pub fn interrupt_now(sim: &mut Sim, unit: UnitId) -> Option<Order> {
    let in_combat = sim.party_engaged(unit);
    if !in_combat || sim.units[unit.index()].hero_mode == HeroMode::AvoidCombat {
        return None;
    }
    let focus = sim.units[unit.index()].focus;
    let situation = Situation {
        focus,
        in_combat,
        no_offence: false,
        locked: focus.is_some(),
        interrupts_only: true,
    };
    let (slot, target) = sim.best_skill(unit, &situation)?;
    Some(Order::UseSkill { slot, target })
}

impl Sim {
    /// The party's leader: the human slot.
    pub fn party_leader(&self) -> Option<UnitId> {
        self.units
            .iter()
            .find(|u| u.kind == UnitKind::Slot(SlotKind::Human))
            .map(|u| u.id)
    }

    /// Whether the party is fighting near this unit: a foe group has
    /// noticed the party and one of its members is close, or a foe is within
    /// aggro range.
    pub fn party_engaged(&self, unit: UnitId) -> bool {
        let me = self.units[unit.index()].pos;
        let aggro = self.fight.tunables.aggro_range;
        let reach = aggro
            + self
                .fight
                .core
                .gwinches(gwsim_data::core::RangeBand::Casting);
        self.units.iter().any(|u| {
            u.alive()
                && u.team == Team::Foes
                && !u.kind.is_summoned()
                && (u.pos.within(me, aggro)
                    || (u.pos.within(me, reach)
                        && u.group.is_some_and(|g| {
                            self.aggroed.get(usize::from(g)).copied().unwrap_or(false)
                        })))
        })
    }

    /// The hero's target (AI-H1): called, then the player's, then its own
    /// previous target, then the weakest foe engaging the party.
    pub fn hero_focus(&mut self, unit: UnitId, in_combat: bool) -> Option<UnitId> {
        let alive = |sim: &Sim, id: Option<UnitId>| id.filter(|t| sim.units[t.index()].alive());
        let chosen = alive(self, self.units[unit.index()].locked_target)
            .or_else(|| alive(self, self.called_target))
            .or_else(|| {
                let leader = self.party_leader()?;
                alive(self, self.units[leader.index()].focus)
            })
            .or_else(|| alive(self, self.units[unit.index()].focus).filter(|_| in_combat))
            .or_else(|| {
                if !in_combat {
                    return None;
                }
                let me = self.units[unit.index()].pos;
                let reach = self.fight.tunables.aggro_range
                    + self
                        .fight
                        .core
                        .gwinches(gwsim_data::core::RangeBand::Casting);
                self.units
                    .iter()
                    .filter(|u| {
                        u.alive()
                            && u.team == Team::Foes
                            && !u.kind.is_summoned()
                            && u.pos.within(me, reach)
                    })
                    .min_by(|a, b| {
                        a.armor[1]
                            .base
                            .cmp(&b.armor[1].base)
                            .then(
                                self.health_fraction(a.id)
                                    .total_cmp(&self.health_fraction(b.id)),
                            )
                            .then(a.id.cmp(&b.id))
                    })
                    .map(|u| u.id)
            });
        self.units[unit.index()].focus = chosen;
        chosen
    }

    /// Walks to the hero's formation point beside the leader, if it has
    /// drifted.
    pub fn follow_leader(&self, unit: UnitId) -> Option<Order> {
        let leader = self.party_leader()?;
        if leader == unit || !self.units[leader.index()].alive() {
            return None;
        }
        let me = &self.units[unit.index()];
        let point = self.units[leader.index()].pos + me.home;
        if me.pos.within(point, FOLLOW_SLACK) {
            return None;
        }
        Some(Order::MoveTo(point))
    }
}
