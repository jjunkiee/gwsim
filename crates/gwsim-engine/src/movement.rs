//! Movement and body-blocking (T3.2.3, T3.2.4, §10.3).
//!
//! A unit's speed is the base speed (A-001) times the product of its
//! movement modifiers, capped at +34% and −50% (ENG-33). Knocked-down units
//! do not move, and starting to move cancels an activation. Units are circles
//! of radius A-002; a unit walking into a **hostile** circle stops at contact
//! and slides along it, while friendly units pass through each other (Body
//! block). Collisions are resolved in `UnitId` order, so the result never
//! depends on anything but the inputs.

use gwsim_data::dsl::Stat;

use crate::geom::Vec2;
use crate::sim::Sim;
use crate::time::TICK_MS;
use crate::unit::{Action, MoveGoal, PendingUse, UnitId};

impl Sim {
    /// A unit's movement speed now, in gwinches per second.
    pub fn movement_speed(&self, unit: UnitId) -> f32 {
        self.fight.tunables.base_speed * self.stat_multiplier(unit, Stat::MovementSpeed) as f32
    }

    /// Moves every unit with somewhere to go, one tick's worth.
    pub(crate) fn move_units(&mut self) {
        let dt = TICK_MS as f32 / 1000.0;
        for index in 0..self.units.len() {
            let unit = UnitId(index as u16);
            let u = &self.units[index];
            if !u.alive() || u.stationary || u.knocked_down() {
                self.units[index].velocity = Vec2::ZERO;
                continue;
            }
            let Some(goal) = u.goal else {
                self.units[index].velocity = Vec2::ZERO;
                continue;
            };
            let position = u.pos;
            let destination = match goal {
                MoveGoal::Point(point) => {
                    if position.within(point, 1.0) {
                        self.units[index].goal = None;
                        self.units[index].velocity = Vec2::ZERO;
                        continue;
                    }
                    point
                }
                MoveGoal::Approach {
                    unit: target,
                    range,
                    then,
                } => {
                    let other = &self.units[target.index()];
                    if !other.alive() {
                        self.units[index].goal = None;
                        self.units[index].velocity = Vec2::ZERO;
                        continue;
                    }
                    if position.within(other.pos, range) {
                        self.units[index].goal = None;
                        self.units[index].velocity = Vec2::ZERO;
                        self.arrive(unit, target, then);
                        continue;
                    }
                    // Aim for the edge of the range, not the target's centre.
                    let towards = (other.pos - position).normalised();
                    other.pos - towards * (range * 0.95)
                }
                MoveGoal::AwayFrom(point) => position + (position - point).normalised() * 1000.0,
            };

            // Moving cancels an activation (§10.5).
            if self.units[index].activating().is_some() {
                self.cancel(unit);
            }
            if matches!(self.units[index].action, Action::Aftercast { .. }) {
                continue;
            }

            self.touch(1);
            let speed = self.movement_speed(unit);
            let offset = destination - position;
            let distance = offset.length();
            let step = (speed * dt).min(distance);
            let direction = offset.normalised();
            let mut next = position + direction * step;
            next = self.block(unit, position, next);
            let u = &mut self.units[index];
            u.velocity = (next - position) * (1.0 / dt);
            if direction.length_squared() > 0.0 {
                u.facing = direction;
            }
            u.pos = next;
        }
    }

    /// Stops a move at contact with any hostile unit's circle, sliding along
    /// it (A-002).
    fn block(&mut self, unit: UnitId, from: Vec2, mut to: Vec2) -> Vec2 {
        let team = self.units[unit.index()].team;
        let radius = self.units[unit.index()].radius;
        let mut touched = false;
        for other in &self.units {
            if other.id == unit || other.team == team || !other.alive() {
                continue;
            }
            let contact = radius + other.radius;
            let offset = to - other.pos;
            if offset.length_squared() < contact * contact {
                // Only block movement into the circle, not out of it.
                if (from - other.pos).length_squared() >= offset.length_squared() {
                    let push = offset.normalised();
                    let push = if push.length_squared() == 0.0 {
                        (from - other.pos).normalised()
                    } else {
                        push
                    };
                    to = other.pos + push * contact;
                    touched = true;
                }
            }
        }
        if touched {
            self.touch(2);
        }
        to
    }

    /// Arriving in range: start what the approach was for.
    fn arrive(&mut self, unit: UnitId, target: UnitId, then: PendingUse) {
        match then {
            PendingUse::Nothing => {}
            PendingUse::Skill { slot } => {
                let _ = self.use_skill(unit, slot, crate::unit::Target::Unit(target));
            }
            PendingUse::Attack => {
                let u = &mut self.units[unit.index()];
                if matches!(u.action, Action::Idle | Action::Attacking { .. }) {
                    u.action = Action::Attacking { target };
                }
            }
        }
    }
}
