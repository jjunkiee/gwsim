//! Controllers: what each unit decides to do (T3.1.5, §11.1).
//!
//! A controller is asked on every tick once its unit is free and its
//! reaction delay has passed since its last action ended. It returns an
//! [`Order`]; the fight carries it out through the skill pipeline, so no
//! controller can change state directly. Controllers are an enum rather than
//! trait objects so a fight can be cloned for each run and dispatch stays
//! cheap and deterministic.

pub mod foe;
pub mod hero;
pub mod plan;
pub mod profile;
pub mod skills;

use crate::log::{LogEvent, LogKind};
use crate::pipeline::Order;
use crate::sim::Sim;
use crate::unit::{Action, UnitId};

pub use plan::{ResolvedPlan, ResolvedRule};

/// Who decides for a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Controller {
    /// Does nothing: training dummies.
    Idle,
    /// A human slot following its priority plan (§11.5).
    Plan { plan: u16 },
    /// A foe (WP4.4).
    Foe,
    /// A hero or henchman (WP4.5).
    Hero,
    /// A minion (WP4.5).
    Minion,
    /// A spirit (WP4.5).
    Spirit,
}

/// How long a unit waits after an action before its next decision.
pub fn reaction_delay(sim: &mut Sim, unit: UnitId) -> u32 {
    match sim
        .controllers
        .get(unit.index())
        .copied()
        .unwrap_or(Controller::Idle)
    {
        Controller::Plan { .. } => {
            sim.touch(12);
            sim.fight.tunables.human_reaction_ms
        }
        Controller::Foe => {
            sim.touch(10);
            if sim.fight.hard_mode {
                sim.fight.tunables.foe_reaction_hm_ms
            } else {
                sim.fight.tunables.foe_reaction_ms
            }
        }
        Controller::Hero => {
            sim.touch(11);
            sim.fight.tunables.hero_reaction_ms
        }
        Controller::Idle | Controller::Minion | Controller::Spirit => 0,
    }
}

impl Sim {
    /// Asks every free unit's controller for an order.
    pub(crate) fn decide(&mut self) {
        for index in 0..self.units.len() {
            let unit = UnitId(index as u16);
            let u = &self.units[index];
            if !u.alive() || u.goal.is_some() {
                continue;
            }
            if !matches!(u.action, Action::Idle | Action::Attacking { .. }) {
                continue;
            }
            let controller = self
                .controllers
                .get(index)
                .copied()
                .unwrap_or(Controller::Idle);
            if self.now < u.next_decision_at {
                // Heroes interrupt without a reaction delay (A-011): they
                // look again the moment a foe starts a skill.
                let just_cast = self
                    .foe_cast_started
                    .is_some_and(|at| self.now.ms().saturating_sub(at.ms()) < crate::time::TICK_MS);
                if controller == Controller::Hero
                    && just_cast
                    && let Some(order) = hero::interrupt_now(self, unit)
                {
                    self.order(unit, order);
                }
                continue;
            }
            // A controller with nothing to do looks again after its reaction
            // delay, unless something changes first (T4.4.2's cadence).
            if matches!(controller, Controller::Foe | Controller::Hero) {
                let delay = reaction_delay(self, unit);
                self.units[index].next_decision_at = self.now.plus(delay);
            }
            let order = match controller {
                Controller::Idle => None,
                Controller::Plan { plan } => plan::decide(self, unit, plan),
                Controller::Foe => foe::decide(self, unit),
                Controller::Hero => hero::decide(self, unit),
                Controller::Minion | Controller::Spirit => self.decide_by_rule(unit, controller),
            };
            if let Some(order) = order {
                if self.logging() {
                    self.log_event(
                        LogEvent::new(self.now, LogKind::Decision)
                            .source(unit)
                            .detail(&format!("{order:?}")),
                    );
                }
                self.order(unit, order);
            }
        }
    }

    /// The controllers WP4.4 and WP4.5 replace. Until then they attack the
    /// nearest hostile unit in aggro range, which is enough to drive a fight.
    /// Spirits never move and attack only what their weapon reaches; minions
    /// fight their master's target, or follow their master (Minion).
    pub fn decide_by_rule(&mut self, unit: UnitId, controller: Controller) -> Option<Order> {
        let target = match controller {
            Controller::Spirit => {
                let reach = self.units[unit.index()].weapon.as_ref()?.range;
                self.nearest_hostile(unit, reach)?
            }
            Controller::Minion => {
                let u = &self.units[unit.index()];
                let master = u.master?;
                let masters_target = self.units[master.index()]
                    .attack_target
                    .filter(|t| self.units[t.index()].alive());
                match masters_target
                    .or_else(|| self.nearest_hostile(unit, self.fight.tunables.aggro_range))
                {
                    Some(target) => target,
                    None => {
                        let home = self.units[master.index()].pos;
                        let follow = self.fight.tunables.collision_radius * 4.0;
                        if u.pos.within(home, follow) || !self.units[master.index()].alive() {
                            return None;
                        }
                        return Some(Order::MoveTo(home));
                    }
                }
            }
            _ => self.nearest_hostile(unit, self.fight.tunables.aggro_range)?,
        };
        if self.units[unit.index()].attack_target == Some(target) {
            return None;
        }
        Some(Order::Attack(target))
    }

    /// The nearest living hostile unit within a radius, lowest id first on a
    /// tie.
    pub fn nearest_hostile(&self, unit: UnitId, radius: f32) -> Option<UnitId> {
        let me = &self.units[unit.index()];
        self.units
            .iter()
            .filter(|u| u.alive() && u.team != me.team && u.pos.within(me.pos, radius))
            .min_by(|a, b| {
                a.pos
                    .distance_squared(me.pos)
                    .total_cmp(&b.pos.distance_squared(me.pos))
                    .then(a.id.cmp(&b.id))
            })
            .map(|u| u.id)
    }
}
