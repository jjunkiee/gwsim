//! `FoeAi`: foes aggro, pick targets, use their skills and move as the wiki
//! documents (WP4.4, AI-F1 to AI-F8).
//!
//! - **Aggro (AI-F1):** a group stands idle until a party creature comes
//!   within the aggro range (A-009) of any member, or a member is attacked
//!   or damaged; then the whole group engages.
//! - **Targeting (AI-F2):** a foe goes for the weakest target in reach —
//!   low armor, low health, close — and sticks to it until it dies. Spirits
//!   are a last resort. In hard mode foes ball up less, which a small random
//!   spread from the AI stream models (AI-F6).
//! - **Skills (AI-F3):** the shared evaluator ([`super::skills`]), aimed at
//!   the chosen target where the skill is offensive.
//! - **Movement (AI-F5, AI-F8):** casters close to casting range and stand
//!   still to cast (the pipeline's approach); melee foes chase; stationary
//!   foes never move; kiters step back from melee.

use crate::ai::skills::Situation;
use crate::pipeline::Order;
use crate::rng::Purpose;
use crate::sim::Sim;
use crate::unit::{Team, UnitId, UnitKind};

/// Target scoring weights (AI-F2): armor per 100, health fraction, distance
/// per this many gwinches, and the penalty for a spirit.
const DISTANCE_SCALE: f32 = 1500.0;
const SPIRIT_PENALTY: f64 = 0.5;
/// Hard mode's spread of target choice, as the largest random addition.
const HARD_MODE_SPREAD: f64 = 0.25;
/// How close a melee attacker must be before a kiter steps away.
const KITE_TRIGGER: f32 = 200.0;
const KITE_STEP: f32 = 400.0;

/// The foe's next order, if any.
pub fn decide(sim: &mut Sim, unit: UnitId) -> Option<Order> {
    if !sim.group_aggroed(unit) {
        return None;
    }
    let target = sim.foe_target(unit)?;

    if let Some(order) = sim.kite(unit) {
        return Some(order);
    }

    let situation = Situation {
        focus: Some(target),
        in_combat: true,
        no_offence: false,
        locked: false,
        interrupts_only: false,
    };
    if let Some((slot, target)) = sim.best_skill(unit, &situation) {
        return Some(Order::UseSkill { slot, target });
    }
    if sim.units[unit.index()].attack_target != Some(target)
        && sim.units[unit.index()].weapon.is_some()
    {
        return Some(Order::Attack(target));
    }
    None
}

impl Sim {
    /// Whether a foe's group has noticed the party, noticing it now if a
    /// party creature is in range or a member has been in combat (AI-F1).
    pub fn group_aggroed(&mut self, unit: UnitId) -> bool {
        let Some(group) = self.units[unit.index()].group.map(usize::from) else {
            return true;
        };
        if self.aggroed.len() <= group {
            self.aggroed.resize(group + 1, false);
        }
        if self.aggroed[group] {
            return true;
        }
        self.touch(9);
        let range = self.fight.tunables.aggro_range;
        let noticed = self.units.iter().any(|member| {
            member.group.map(usize::from) == Some(group)
                && (member.last_combat.is_some()
                    || (member.alive()
                        && self.units.iter().any(|u| {
                            u.alive() && u.team == Team::Party && u.pos.within(member.pos, range)
                        })))
        });
        if noticed {
            self.aggroed[group] = true;
            if self.engaged_at.is_none() {
                self.engaged_at = Some(self.now);
            }
        }
        noticed
    }

    /// The foe's target: its current one while it lives, else the weakest
    /// party creature in reach (AI-F2).
    pub fn foe_target(&mut self, unit: UnitId) -> Option<UnitId> {
        if let Some(focus) = self.units[unit.index()].focus
            && self.units[focus.index()].alive()
        {
            return Some(focus);
        }
        let me = self.units[unit.index()].pos;
        let reach = self.fight.tunables.aggro_range
            + self
                .fight
                .core
                .gwinches(gwsim_data::core::RangeBand::Casting);
        let candidates: Vec<UnitId> = self
            .units
            .iter()
            .filter(|u| u.alive() && u.team == Team::Party && u.pos.within(me, reach))
            .map(|u| u.id)
            .collect();
        let hard = self.fight.hard_mode;
        let mut best: Option<(f64, UnitId)> = None;
        for candidate in candidates {
            let u = &self.units[candidate.index()];
            let armor = f64::from(u.armor[1].base) / 100.0;
            let distance = f64::from(u.pos.distance(me) / DISTANCE_SCALE);
            let spirit = if u.kind == UnitKind::Spirit {
                SPIRIT_PENALTY
            } else {
                0.0
            };
            let spread = if hard {
                self.streams.unit(unit, Purpose::Ai).unit_f64() * HARD_MODE_SPREAD
            } else {
                0.0
            };
            let score = armor + self.health_fraction(candidate) + distance + spirit + spread;
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, candidate));
            }
        }
        let target = best.map(|(_, id)| id)?;
        self.units[unit.index()].focus = Some(target);
        Some(target)
    }

    /// A kiter's step back from a melee attacker (AI-F5).
    fn kite(&self, unit: UnitId) -> Option<Order> {
        let me = &self.units[unit.index()];
        if !me.kiter || me.stationary {
            return None;
        }
        let threat = self.units.iter().find(|u| {
            u.alive()
                && u.team != me.team
                && u.weapon
                    .as_ref()
                    .is_some_and(|w| w.projectile_speed.is_none())
                && u.pos.within(me.pos, KITE_TRIGGER)
        })?;
        let away = (me.pos - threat.pos).normalised();
        Some(Order::MoveTo(me.pos + away * KITE_STEP))
    }
}
