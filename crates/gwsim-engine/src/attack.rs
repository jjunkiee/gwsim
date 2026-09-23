//! Auto-attacks, attack skills and projectiles (T3.4.10, T3.2.5, T3.5.4).
//!
//! An attack cycle lasts the weapon's interval, scaled by attack speed
//! (capped at +33% and −50%). The hit lands half-way through (A-035), which
//! is also where the wiki puts the hit of an attack skill. Ranged weapons
//! launch a projectile aimed where the target will be (leading), which
//! misses if the target changes course enough.
//!
//! Hit resolution follows the wiki's order: miss (Blind's 90%, multiplied
//! with other miss chances), then block (multiplicative), then hit location,
//! then the critical roll. All rolls come from the attacker's streams.

use gwsim_data::core::{Attribute, Condition, DamageType, Profession};
use gwsim_data::dsl::{Action as DslAction, Event, Stat, Value};

use crate::combat;
use crate::damage::DamageInfo;
use crate::exec::ExecCtx;
use crate::geom::Vec2;
use crate::log::{LogEvent, LogKind};
use crate::rng::Purpose;
use crate::sim::{Fired, Sim};
use crate::time::{EventKind, SimTime};
use crate::unit::{Action, MoveGoal, PendingUse, Target, UnitId};

/// A projectile in flight.
#[derive(Debug, Clone, PartialEq)]
pub struct Projectile {
    pub id: u32,
    pub source: UnitId,
    pub target: UnitId,
    /// Where the projectile was aimed: the target's predicted position.
    pub aim: Vec2,
    pub impact_at: SimTime,
    /// The attack skill it carries, if any, and its bonus damage.
    pub skill: Option<u16>,
    pub bonus: f64,
    pub unblockable: bool,
    /// What the attack skill does if it hits, run when it lands.
    pub on_hit: Option<Box<OnHit>>,
}

/// An attack skill's effects that wait for its hit (T3.4.10): conditions,
/// interrupts, stance removal. Blocked or missed, they never happen.
#[derive(Debug, Clone, PartialEq)]
pub struct OnHit {
    pub actions: Vec<DslAction>,
    pub ctx: ExecCtx,
}

/// The intercept time for a projectile from `from` at `speed` against a
/// target at `to` moving at `velocity`, or [`None`] if it can never catch up.
pub fn intercept_time(from: Vec2, speed: f32, to: Vec2, velocity: Vec2) -> Option<f32> {
    let offset = to - from;
    let a = velocity.dot(velocity) - speed * speed;
    let b = 2.0 * offset.dot(velocity);
    let c = offset.dot(offset);
    if a.abs() < 1e-6 {
        if b.abs() < 1e-6 {
            return None;
        }
        let t = -c / b;
        return (t > 0.0).then_some(t);
    }
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let (t1, t2) = ((-b - root) / (2.0 * a), (-b + root) / (2.0 * a));
    let t = [t1, t2]
        .into_iter()
        .filter(|t| *t > 0.0)
        .fold(f32::INFINITY, f32::min);
    t.is_finite().then_some(t)
}

impl Sim {
    /// Starts swings for every unit attacking something in range (T3.4.10).
    pub(crate) fn drive_attacks(&mut self) {
        for index in 0..self.units.len() {
            let unit = UnitId(index as u16);
            let Action::Attacking { target } = self.units[index].action else {
                continue;
            };
            if !self.units[target.index()].alive() {
                let u = &mut self.units[index];
                u.action = Action::Idle;
                u.attack_target = None;
                u.swing_hits_at = None;
                continue;
            }
            let Some(weapon) = self.units[index].weapon.clone() else {
                continue;
            };
            let distance = self.units[index]
                .pos
                .distance(self.units[target.index()].pos);
            if distance > weapon.range {
                if !self.units[index].stationary && self.units[index].goal.is_none() {
                    self.units[index].goal = Some(MoveGoal::Approach {
                        unit: target,
                        range: weapon.range,
                        then: PendingUse::Attack,
                    });
                }
                continue;
            }
            if self.units[index].swing_hits_at.is_some()
                || self.now < self.units[index].next_swing_at
            {
                continue;
            }
            let interval = (f64::from(weapon.interval_ms)
                * self.stat_multiplier(unit, Stat::AttackSpeed))
            .round() as u32;
            self.touch(35);
            let hit_after =
                (f64::from(interval) * self.fight.tunables.attack_hit_fraction).round() as u32;
            let now = self.now;
            let u = &mut self.units[index];
            u.next_swing_at = now.plus(interval);
            u.swing_hits_at = Some(now.plus(hit_after));
            let generation = u.action_generation;
            self.queue.schedule(
                now.plus(hit_after),
                EventKind::AttackHit { unit, generation },
            );
            self.mark_combat(unit);
            self.fire(Fired {
                event: Event::OnAttack,
                subject: unit,
                other: Some(target),
                skill: None,
                amount: 0.0,
            });
        }
    }

    /// A swing reaches its hit.
    pub(crate) fn attack_hit(&mut self, unit: UnitId, generation: u32) {
        // Whatever happens to this swing, it is over: a swing a skill or an
        // interrupt cut short must not block the next one.
        let now = self.now;
        let u = &mut self.units[unit.index()];
        if u.swing_hits_at.is_some_and(|at| at <= now) {
            u.swing_hits_at = None;
        }
        if u.action_generation != generation {
            return;
        }
        let Action::Attacking { target } = u.action else {
            return;
        };
        self.units[unit.index()].swing_hits_at = None;
        self.launch_or_strike(unit, target, None, 0.0, false, None, 1.0);
    }

    /// Fires a projectile for a ranged weapon, or strikes at once for melee.
    #[allow(clippy::too_many_arguments)]
    fn launch_or_strike(
        &mut self,
        unit: UnitId,
        target: UnitId,
        skill: Option<u16>,
        bonus: f64,
        unblockable: bool,
        on_hit: Option<Box<OnHit>>,
        speed_factor: f64,
    ) {
        let Some(weapon) = self.units[unit.index()].weapon.clone() else {
            return;
        };
        match weapon.projectile_speed {
            Some(speed) => {
                self.touch(3);
                let speed = speed
                    * (self.stat_multiplier(unit, Stat::ProjectileSpeed) * speed_factor) as f32;
                let from = self.units[unit.index()].pos;
                let to = self.units[target.index()].pos;
                let velocity = self.units[target.index()].velocity;
                let time = intercept_time(from, speed, to, velocity)
                    .unwrap_or_else(|| from.distance(to) / speed);
                let aim = to + velocity * time;
                let impact_at = self.now.plus((time * 1000.0).round() as u32);
                let id = self.projectiles.len() as u32;
                self.projectiles.push(Projectile {
                    id,
                    source: unit,
                    target,
                    aim,
                    impact_at,
                    skill,
                    bonus,
                    unblockable,
                    on_hit,
                });
                self.queue
                    .schedule(impact_at, EventKind::ProjectileImpact { projectile: id });
            }
            None => self.resolve_attack(unit, target, skill, bonus, unblockable, on_hit),
        }
    }

    /// A projectile lands: a hit if the target is still near the aim point.
    pub(crate) fn projectile_impact(&mut self, id: u32) {
        let Some(projectile) = self.projectiles.get(id as usize).cloned() else {
            return;
        };
        let target = &self.units[projectile.target.index()];
        if !target.alive() {
            return;
        }
        let tolerance = self.fight.tunables.collision_radius * 2.0;
        let landed = target.pos.within(projectile.aim, tolerance);
        self.touch(2);
        if landed {
            self.resolve_attack(
                projectile.source,
                projectile.target,
                projectile.skill,
                projectile.bonus,
                projectile.unblockable,
                projectile.on_hit,
            );
        } else {
            self.log_event(
                LogEvent::new(self.now, LogKind::AttackMissed)
                    .source(projectile.source)
                    .target(projectile.target)
                    .detail("dodged"),
            );
        }
    }

    /// Resolves one attack: miss, block, location, critical, damage.
    pub fn resolve_attack(
        &mut self,
        attacker: UnitId,
        target: UnitId,
        skill: Option<u16>,
        bonus: f64,
        unblockable: bool,
        on_hit: Option<Box<OnHit>>,
    ) {
        if !self.units[attacker.index()].alive() || !self.units[target.index()].alive() {
            return;
        }
        let Some(weapon) = self.units[attacker.index()].weapon.clone() else {
            return;
        };

        // Miss: Blind's 90%, multiplied with any other miss chance.
        let miss = if self.has_condition(attacker, Condition::Blind) {
            0.9
        } else {
            0.0
        };
        if miss > 0.0 && self.streams.unit(attacker, Purpose::Hits).chance(miss) {
            self.log_event(
                LogEvent::new(self.now, LogKind::AttackMissed)
                    .source(attacker)
                    .target(target),
            );
            self.fire(Fired {
                event: Event::OnMiss,
                subject: attacker,
                other: Some(target),
                skill,
                amount: 0.0,
            });
            return;
        }

        // Block: multiplicative, uncapped.
        let block = self.stat_multiplier(target, Stat::BlockChance);
        if !unblockable && block > 0.0 && self.streams.unit(attacker, Purpose::Hits).chance(block) {
            self.log_event(
                LogEvent::new(self.now, LogKind::AttackBlocked)
                    .source(attacker)
                    .target(target),
            );
            self.fire(Fired {
                event: Event::OnBlocked,
                subject: target,
                other: Some(attacker),
                skill,
                amount: 0.0,
            });
            return;
        }

        let roll = self.streams.unit(attacker, Purpose::Hits).unit_f64();
        let piece = combat::hit_location(roll);
        self.touch(14);

        let level = self.units[attacker.index()].level;
        let weapon_rank = weapon
            .mastery
            .map(|a| self.rank_of(attacker, a))
            .unwrap_or(0);
        let chance = combat::critical_chance(level, weapon_rank, self.units[target.index()].level);
        let critical = !self.critical_immune(target)
            && self.streams.unit(attacker, Purpose::Crits).chance(chance);

        let base = if critical {
            f64::from(weapon.damage.1)
        } else {
            f64::from(
                self.streams
                    .unit(attacker, Purpose::Hits)
                    .range_inclusive(weapon.damage.0, weapon.damage.1),
            )
        };
        // Weakness takes two thirds off the weapon's base damage, not bonuses.
        let base = if self.has_condition(attacker, Condition::Weakness) {
            base * (1.0 - 0.66)
        } else {
            base
        };
        let strike = match weapon.mastery {
            Some(_) => combat::strike_level(weapon_rank, level),
            None => combat::skill_strike_level(level),
        };
        // Strength: +1% armor penetration per rank on attack skills.
        let mut penetration = 0.0;
        if skill.is_some() && self.units[attacker.index()].professions.0 == Profession::Warrior {
            penetration = f64::from(self.rank_of(attacker, Attribute::Strength)) / 100.0;
        }
        if weapon.slug == "hornbow" {
            penetration += 0.10;
        }
        let strike = if critical { strike + 20.0 } else { strike };
        // Spirit attacks ignore armor, and their crits add nothing (Spirit).
        let weapon_part = if weapon.armor_ignoring {
            base
        } else {
            let armor = self.armor_against(target, weapon.damage_type, piece, penetration);
            combat::damage_packet(base, strike, armor)
        };
        if critical {
            self.log_event(
                LogEvent::new(self.now, LogKind::CriticalHit)
                    .source(attacker)
                    .target(target),
            );
        }
        // "+X damage" from an attack skill is armor-ignoring and joins the
        // weapon's damage in one packet (Damage calculation).
        self.deal_damage(
            attacker,
            target,
            weapon_part + bonus,
            DamageInfo {
                kind: Some(weapon.damage_type),
                armor_ignoring: true,
                skill,
                attack: true,
                strike_level: strike,
                penetration,
                critical,
                piece: Some(piece),
            },
        );

        // One strike of adrenaline per hit.
        self.gain_adrenaline(attacker, crate::unit::ADRENALINE_PER_STRIKE);
        self.fire(Fired {
            event: Event::OnHit,
            subject: attacker,
            other: Some(target),
            skill,
            amount: 0.0,
        });
        self.fire(Fired {
            event: Event::OnStruck,
            subject: target,
            other: Some(attacker),
            skill,
            amount: 0.0,
        });
        if let Some(on_hit) = on_hit {
            let OnHit { actions, mut ctx } = *on_hit;
            self.execute(&actions, &mut ctx);
        }
    }

    /// An attack skill: a weapon attack whose top-level `Damage` actions on
    /// the target are its "+X damage" bonus, with the rest of its effects
    /// applied after the hit.
    pub(crate) fn attack_skill(
        &mut self,
        unit: UnitId,
        target: Target,
        skill: u16,
        effects: &[DslAction],
        ctx: &mut ExecCtx,
    ) {
        let Some(target_unit) = target.unit() else {
            self.execute(effects, ctx);
            return;
        };
        // Three kinds of action: the "+X damage" bonus joins the weapon's
        // packet; gating actions (unblockable, a faster projectile) shape the
        // attack before it flies; everything else waits for the hit.
        let mut bonus = 0.0;
        let mut gating = Vec::new();
        let mut rest = Vec::new();
        for action in effects {
            match action {
                DslAction::Damage {
                    to: gwsim_data::dsl::Selector::TargetFoe | gwsim_data::dsl::Selector::Target,
                    amount,
                    ..
                } => bonus += self.eval_value(amount, ctx),
                DslAction::ModifyStat {
                    stat: Stat::ProjectileSpeed,
                    amount,
                    ..
                } => ctx.projectile_speed *= 1.0 + self.eval_value(amount, ctx) / 100.0,
                other if gates_attack(other) => gating.push(other.clone()),
                other => rest.push(other.clone()),
            }
        }
        self.execute(&gating, ctx);
        let on_hit = (!rest.is_empty()).then(|| {
            Box::new(OnHit {
                actions: rest,
                ctx: ctx.clone(),
            })
        });
        if self.units[target_unit.index()].alive() {
            self.launch_or_strike(
                unit,
                target_unit,
                Some(skill),
                bonus,
                ctx.unblockable,
                on_hit,
                ctx.projectile_speed,
            );
        }
        // An attack skill resets the swing, so the next auto-attack follows it.
        let now = self.now;
        let u = &mut self.units[unit.index()];
        u.next_swing_at = now;
        u.attack_target = Some(target_unit);
    }

    /// The damage type of a caster weapon for a profession (A-037).
    pub fn caster_weapon_type(&mut self, profession: Profession) -> DamageType {
        self.touch(37);
        Sim::caster_damage_type(profession)
    }

    /// Reads a skill's first scaled value, for handlers that need a number.
    pub fn first_scaled(&self, skill: u16) -> Option<Value> {
        self.fight.skills[usize::from(skill)]
            .skill
            .extracted
            .scaled
            .first()
            .map(|n| Value::Scaled(n.r0, n.r15))
    }
}

/// Whether an attack skill's action shapes the attack itself, and so runs
/// before it flies: `SetUnblockable`, alone or behind an `If`.
fn gates_attack(action: &DslAction) -> bool {
    match action {
        DslAction::SetUnblockable => true,
        DslAction::Control(control) => match control.as_ref() {
            gwsim_data::dsl::Control::If {
                then, otherwise, ..
            } => then
                .iter()
                .chain(otherwise.iter())
                .all(|a| matches!(a, DslAction::SetUnblockable)),
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stationary_target_is_reached_in_distance_over_speed() {
        let t = intercept_time(Vec2::ZERO, 1000.0, Vec2::new(500.0, 0.0), Vec2::ZERO).unwrap();
        assert!((t - 0.5).abs() < 1e-5);
    }

    #[test]
    fn a_moving_target_is_led() {
        // Target moving away at 288 from 1000 away, projectile at 1800.
        let t = intercept_time(
            Vec2::ZERO,
            1800.0,
            Vec2::new(1000.0, 0.0),
            Vec2::new(288.0, 0.0),
        )
        .unwrap();
        assert!((t - 1000.0 / (1800.0 - 288.0)).abs() < 1e-4);
    }

    #[test]
    fn a_target_faster_than_the_projectile_cannot_be_caught() {
        assert!(
            intercept_time(
                Vec2::ZERO,
                100.0,
                Vec2::new(500.0, 0.0),
                Vec2::new(200.0, 0.0)
            )
            .is_none()
        );
    }
}
