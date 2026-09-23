//! Using a skill, from the first check to recharge (WP3.4, ENG-10 to ENG-18).
//!
//! The five ways an activation can end, per the wiki's Activation time,
//! Interrupt, Fail and Fizzle pages (ENG-14):
//!
//! | Outcome | Cost | Recharge | Aftercast |
//! | --- | --- | --- | --- |
//! | complete | paid | starts | per type |
//! | interrupted | lost | starts | none |
//! | cancelled (moving, a new order) | paid | none | none |
//! | failed (a prerequisite, or the target lost: A-040) | lost | instant | none |
//! | fizzled (invalid target at the end) | lost | none | none |
//!
//! Knockdown stops an activation through its own path: it is not an
//! interrupt and bypasses interrupt prevention (ENG-35), but the skill still
//! recharges.

use std::sync::Arc;

use gwsim_data::core::{Attribute, Condition, Profession, SkillType};
use gwsim_data::dsl::{Event, Stat};
use gwsim_data::skill::TargetKind;

use crate::combat;
use crate::exec::ExecCtx;
use crate::geom::Vec2;
use crate::log::{LogEvent, LogKind};
use crate::rng::Purpose;
use crate::sim::{Fired, Sim};
use crate::time::EventKind;
use crate::unit::{
    ADRENALINE_PER_STRIKE, Action, ENERGY_SCALE, MoveGoal, PendingUse, Target, Team, UnitId,
    UnitKind,
};

/// Why a skill cannot be used now (T3.4.2). Controllers and the log use
/// these to explain "why didn't the hero cast X".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    Dead,
    NoSkill,
    KnockedDown,
    Recharging,
    Disabled,
    NoEnergy,
    NoAdrenaline,
    NoHealthForSacrifice,
    BadTarget(&'static str),
    Busy,
    FlashDuringActivation,
}

/// Which activations an interrupt can stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptScope {
    /// Any skill or attack.
    Any,
    /// Any skill with an activation time, but not an auto-attack.
    Skill,
    Spell,
    Attack,
    Chant,
}

/// What a controller tells a unit to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Order {
    UseSkill { slot: u8, target: Target },
    Attack(UnitId),
    MoveTo(Vec2),
    Stop,
}

impl Sim {
    // ------------------------------------------------------------ checks

    /// Whether a unit can use a slot on a target now (ENG-10).
    pub fn can_use(&self, unit: UnitId, slot: u8, target: Target) -> Result<(), Invalid> {
        let u = &self.units[unit.index()];
        if !u.alive() {
            return Err(Invalid::Dead);
        }
        let Some(state) = u.bar.get(usize::from(slot)).copied().flatten() else {
            return Err(Invalid::NoSkill);
        };
        let fight_skill = &self.fight.skills[usize::from(state.skill)];
        let kind = fight_skill.skill.kind;
        if u.knocked_down() && !kind.usable_while_knocked_down() {
            return Err(Invalid::KnockedDown);
        }
        if self.now < state.disabled_until {
            return Err(Invalid::Disabled);
        }
        if self.now < state.ready_at {
            return Err(Invalid::Recharging);
        }
        if u.energy_points() < self.energy_cost(unit, state.skill) {
            return Err(Invalid::NoEnergy);
        }
        let adrenaline = i32::from(fight_skill.skill.cost.adrenaline) * ADRENALINE_PER_STRIKE;
        if state.adrenaline < adrenaline {
            return Err(Invalid::NoAdrenaline);
        }
        if fight_skill.skill.flags.needs_corpse && self.nearest_corpse(unit).is_none() {
            return Err(Invalid::BadTarget("no corpse in range"));
        }
        if kind.uses_action_queue() {
            match u.action {
                Action::Activating { .. } if kind.is_a(SkillType::FlashEnchantmentSpell) => {
                    return Err(Invalid::FlashDuringActivation);
                }
                Action::Activating { .. } | Action::Aftercast { .. } => return Err(Invalid::Busy),
                _ => {}
            }
        }
        self.check_target(unit, fight_skill.skill.target, target)
    }

    fn check_target(&self, unit: UnitId, kind: TargetKind, target: Target) -> Result<(), Invalid> {
        let me = &self.units[unit.index()];
        let other = |t: Target| t.unit().map(|id| &self.units[id.index()]);
        match kind {
            TargetKind::SelfOnly | TargetKind::None | TargetKind::Location => Ok(()),
            TargetKind::Foe => match other(target) {
                Some(u) if !u.alive() => Err(Invalid::BadTarget("dead")),
                Some(u) if u.team == me.team => Err(Invalid::BadTarget("not a foe")),
                Some(_) => Ok(()),
                None => Err(Invalid::BadTarget("no target")),
            },
            TargetKind::Ally | TargetKind::AllyOrSelf => match other(target) {
                Some(u) if !u.alive() => Err(Invalid::BadTarget("dead")),
                Some(u) if u.team != me.team => Err(Invalid::BadTarget("not an ally")),
                Some(_) | None => Ok(()),
            },
            TargetKind::OtherAlly => match other(target) {
                Some(u) if u.id == unit => Err(Invalid::BadTarget("must be another ally")),
                Some(u) if !u.alive() => Err(Invalid::BadTarget("dead")),
                Some(u) if u.team != me.team => Err(Invalid::BadTarget("not an ally")),
                Some(_) => Ok(()),
                None => Err(Invalid::BadTarget("no target")),
            },
            TargetKind::Corpse => match other(target) {
                Some(u) if u.alive() => Err(Invalid::BadTarget("not dead")),
                Some(_) => Ok(()),
                None => Err(Invalid::BadTarget("no corpse")),
            },
            TargetKind::Spirit => match other(target) {
                Some(u) if u.kind != UnitKind::Spirit => Err(Invalid::BadTarget("not a spirit")),
                Some(u) if !u.alive() => Err(Invalid::BadTarget("dead")),
                Some(_) => Ok(()),
                None => Err(Invalid::BadTarget("no spirit")),
            },
            TargetKind::Minion => match other(target) {
                Some(u) if u.kind != UnitKind::Minion => Err(Invalid::BadTarget("not a minion")),
                Some(_) => Ok(()),
                None => Err(Invalid::BadTarget("no minion")),
            },
        }
    }

    /// A skill's energy cost for a unit, after the cost-reducing inherent
    /// attributes: Expertise (−4% per rank on Ranger skills, attack skills,
    /// rituals and touch skills) and Mysticism (−4% per rank on Dervish
    /// enchantments), rounded to the nearest point.
    pub fn energy_cost(&self, unit: UnitId, skill: u16) -> i32 {
        let fight_skill = &self.fight.skills[usize::from(skill)];
        let base = f64::from(fight_skill.skill.cost.energy);
        if base == 0.0 {
            return 0;
        }
        let u = &self.units[unit.index()];
        let kind = fight_skill.skill.kind;
        let mut cost = base;
        let expertise = u.base_ranks[Attribute::Expertise.index()];
        if expertise > 0
            && u.professions.0 == Profession::Ranger
            && (fight_skill.skill.profession == Some(Profession::Ranger)
                || kind.is_a(SkillType::AttackSkill)
                || kind.is_a(SkillType::Ritual)
                || fight_skill.skill.flags.touch)
        {
            cost *= (1.0 - 0.04 * f64::from(self.rank_of(unit, Attribute::Expertise))).max(0.0);
        }
        let mysticism = u.base_ranks[Attribute::Mysticism.index()];
        if mysticism > 0
            && u.professions.0 == Profession::Dervish
            && fight_skill.skill.profession == Some(Profession::Dervish)
            && kind.is_a(SkillType::EnchantmentSpell)
        {
            cost *= (1.0 - 0.04 * f64::from(self.rank_of(unit, Attribute::Mysticism))).max(0.0);
        }
        cost = self.adjust_energy_cost(unit, skill, cost);
        cost.round() as i32
    }

    /// The rank a unit uses a skill at: its current rank in the skill's
    /// attribute, or 0.
    pub fn skill_rank(&self, unit: UnitId, skill: u16) -> u8 {
        match self.fight.skills[usize::from(skill)].skill.attribute {
            Some(attribute) => self.rank_of(unit, attribute),
            None => 0,
        }
    }

    /// How far a unit can use a skill from, in gwinches. [`None`] means no
    /// range check applies.
    pub fn skill_range(&self, unit: UnitId, skill: u16) -> Option<f32> {
        let fight_skill = &self.fight.skills[usize::from(skill)];
        if fight_skill.is_attack {
            return self.units[unit.index()].weapon.as_ref().map(|w| w.range);
        }
        let band = fight_skill.skill.range?;
        let mut range = self.fight.core.gwinches(band);
        if fight_skill.skill.flags.half_range {
            range /= 2.0;
        }
        Some(range)
    }

    // ------------------------------------------------------------ orders

    /// Carries out a controller's order.
    pub fn order(&mut self, unit: UnitId, order: Order) {
        if !self.units[unit.index()].alive() {
            return;
        }
        match order {
            Order::UseSkill { slot, target } => {
                let _ = self.use_skill(unit, slot, target);
            }
            Order::Attack(target) => {
                let u = &mut self.units[unit.index()];
                u.attack_target = Some(target);
                u.focus = Some(target);
                if matches!(u.action, Action::Idle | Action::Attacking { .. }) {
                    u.action = Action::Attacking { target };
                }
            }
            Order::MoveTo(point) => {
                if self.units[unit.index()].activating().is_some() {
                    self.cancel(unit);
                }
                self.units[unit.index()].goal = Some(MoveGoal::Point(point));
            }
            Order::Stop => {
                let u = &mut self.units[unit.index()];
                u.goal = None;
                u.attack_target = None;
                if matches!(u.action, Action::Attacking { .. }) {
                    u.action = Action::Idle;
                }
            }
        }
    }

    /// Uses a skill, walking into range first if needed (ENG-11).
    pub fn use_skill(&mut self, unit: UnitId, slot: u8, target: Target) -> Result<(), Invalid> {
        self.can_use(unit, slot, target)?;
        let skill = self.units[unit.index()].bar[usize::from(slot)]
            .map(|s| s.skill)
            .ok_or(Invalid::NoSkill)?;
        if let (Some(range), Some(target_unit)) = (self.skill_range(unit, skill), target.unit())
            && target_unit != unit
        {
            let distance = self.units[unit.index()]
                .pos
                .distance(self.units[target_unit.index()].pos);
            if distance > range {
                if self.units[unit.index()].stationary {
                    return Err(Invalid::BadTarget("out of range"));
                }
                self.units[unit.index()].goal = Some(MoveGoal::Approach {
                    unit: target_unit,
                    range,
                    then: PendingUse::Skill { slot },
                });
                return Ok(());
            }
        }
        self.start_activation(unit, slot, target);
        Ok(())
    }

    /// Pays the costs and starts the activation.
    fn start_activation(&mut self, unit: UnitId, slot: u8, target: Target) {
        let skill = self.units[unit.index()].bar[usize::from(slot)]
            .map(|s| s.skill)
            .expect("checked by can_use");
        let fight = Arc::clone(&self.fight);
        let fight_skill = &fight.skills[usize::from(skill)];
        let kind = fight_skill.skill.kind;
        let queued = kind.uses_action_queue();

        // Costs are paid at the start (ENG-13).
        let energy = self.energy_cost(unit, skill);
        {
            let u = &mut self.units[unit.index()];
            u.energy -= energy * ENERGY_SCALE;
            if fight_skill.skill.cost.adrenaline > 0 {
                // Using an adrenal skill empties its own pool and takes a
                // strike from every other (Adrenaline).
                for (index, other) in u.bar.iter_mut().enumerate() {
                    if let Some(other) = other {
                        if index == usize::from(slot) {
                            other.adrenaline = 0;
                        } else {
                            other.adrenaline = (other.adrenaline - ADRENALINE_PER_STRIKE).max(0);
                        }
                    }
                }
            }
            if fight_skill.skill.cost.overcast > 0 {
                u.overcast += i32::from(fight_skill.skill.cost.overcast) * ENERGY_SCALE;
            }
            u.goal = None;
        }
        let max_energy = self.max_energy(unit) * ENERGY_SCALE;
        {
            let u = &mut self.units[unit.index()];
            if u.energy > max_energy {
                u.energy = max_energy;
            }
            if let Some(index) = u.slot_index {
                self.stats.slots[index].energy_spent += i64::from(energy);
                self.stats.slots[index].skills_used += 1;
            }
        }
        self.stats.skills[usize::from(skill)].uses += 1;
        if let Some(t) = target.unit()
            && self.units[t.index()].team != self.units[unit.index()].team
        {
            self.mark_combat(unit);
            // A cast-time skill on a foe draws its group's attention (Aggro).
            if activation_draws_aggro(fight_skill) {
                self.mark_combat(t);
            }
            self.units[unit.index()].focus = Some(t);
        }

        let activation = self.activation_ms(unit, skill);
        if self.logging() {
            let mut event = LogEvent::new(self.now, LogKind::SkillStarted)
                .source(unit)
                .skill(fight_skill.skill.id.get());
            if let Some(t) = target.unit() {
                event = event.target(t);
            }
            self.log_event(event.amount(activation as i32));
        }

        if self.units[unit.index()].team == crate::unit::Team::Foes && activation > 0 {
            self.foe_cast_started = Some(self.now);
        }
        if queued {
            let now = self.now;
            let u = &mut self.units[unit.index()];
            u.action = Action::Activating {
                slot,
                target,
                started: now,
                ends_at: now.plus(activation),
                failed: false,
            };
            u.action_generation = u.action_generation.wrapping_add(1);
        }

        let fired = Fired {
            event: Event::OnSkillActivationStart,
            subject: unit,
            other: target.unit(),
            skill: Some(skill),
            amount: 0.0,
        };
        self.fire(fired);
        if fight_skill.is_spell {
            // A-041: a trigger on "the next spell cast" fires as the cast starts.
            self.touch(41);
            self.fire(Fired {
                event: Event::OnSpellCast,
                ..fired
            });
        }

        if !queued {
            // Shouts, stances and pet attacks happen at once, outside the
            // queue, and leave whatever the unit was doing alone.
            self.complete(unit, slot, target, skill, false);
            return;
        }
        let failed = matches!(
            self.units[unit.index()].action,
            Action::Activating { failed: true, .. }
        );
        if failed {
            self.fail_activation(unit, slot);
        } else if activation == 0 {
            self.finish_activation(unit);
        } else {
            let generation = self.units[unit.index()].action_generation;
            self.queue.schedule(
                self.now.plus(activation),
                EventKind::ActivationEnd { unit, generation },
            );
        }
    }

    /// An activation's end: complete it, or fail it (T3.4.6).
    pub(crate) fn finish_activation(&mut self, unit: UnitId) {
        let Action::Activating {
            slot,
            target,
            failed,
            ..
        } = self.units[unit.index()].action
        else {
            return;
        };
        if failed {
            self.fail_activation(unit, slot);
            return;
        }
        let Some(skill) = self.slot_skill(unit, slot) else {
            return;
        };
        let target_kind = self.fight.skills[usize::from(skill)].skill.target;
        let needs_living = matches!(
            target_kind,
            TargetKind::Foe
                | TargetKind::Ally
                | TargetKind::OtherAlly
                | TargetKind::Spirit
                | TargetKind::Minion
        );
        if needs_living
            && let Some(t) = target.unit()
            && !self.units[t.index()].alive()
        {
            // A-040: the target died during the activation, so the skill fails.
            self.touch(40);
            self.fail_activation(unit, slot);
            return;
        }
        self.complete(unit, slot, target, skill, true);
    }

    /// A completed skill: effects, sacrifice, recharge, aftercast.
    fn complete(&mut self, unit: UnitId, slot: u8, target: Target, skill: u16, queued: bool) {
        let fight = Arc::clone(&self.fight);
        let fight_skill = &fight.skills[usize::from(skill)];

        // Arcane Echo copies a spell before that spell's effects resolve.
        self.fire(Fired {
            event: Event::OnSkillActivationEnd,
            subject: unit,
            other: target.unit(),
            skill: Some(skill),
            amount: 0.0,
        });

        let rank = self.skill_rank(unit, skill);
        let mut ctx = ExecCtx::for_skill(unit, target, skill, Some(slot), rank);
        if let Some(encoding) = &fight_skill.skill.encoding {
            if fight_skill.is_attack {
                self.attack_skill(unit, target, skill, &encoding.effects, &mut ctx);
            } else {
                self.execute(&encoding.effects, &mut ctx);
            }
            if let Some(handler) = fight_skill.handler {
                fight.handlers.get(handler).on_use(self, &mut ctx);
            }
        }
        self.inherent_after_use(unit, target, skill);
        // A successful use: Panic listens for this.
        self.fire(Fired {
            event: Event::OnSkillUsed,
            subject: unit,
            other: target.unit(),
            skill: Some(skill),
            amount: 0.0,
        });
        if !self.units[unit.index()].alive() {
            return;
        }

        // Sacrifice comes after success (ENG-13).
        if fight_skill.skill.cost.sacrifice_pct > 0 && !ctx.waive_sacrifice {
            self.sacrifice(unit, f64::from(fight_skill.skill.cost.sacrifice_pct));
            if !self.units[unit.index()].alive() {
                return;
            }
        }

        // Recharge starts now, unless a copy took over the slot meanwhile.
        let recharge = self.recharge_ms(unit, skill);
        let recharge = self.adjust_recharge(unit, skill, recharge);
        let now = self.now;
        if let Some(state) = self.units[unit.index()].bar[usize::from(slot)].as_mut()
            && state.skill == skill
        {
            state.ready_at = now.plus(recharge);
        }
        self.log_event(
            LogEvent::new(now, LogKind::SkillCompleted)
                .source(unit)
                .skill(fight_skill.skill.id.get())
                .amount(recharge as i32),
        );

        if !queued {
            return;
        }
        let aftercast = fight_skill.aftercast_ms;
        let u = &mut self.units[unit.index()];
        if aftercast > 0 {
            u.action = Action::Aftercast {
                ends_at: now.plus(aftercast),
            };
            u.action_generation = u.action_generation.wrapping_add(1);
            let generation = u.action_generation;
            self.queue.schedule(
                now.plus(aftercast),
                EventKind::AftercastEnd { unit, generation },
            );
        } else {
            u.action = Action::Idle;
            u.action_generation = u.action_generation.wrapping_add(1);
            self.after_action(unit);
        }
    }

    /// A failed skill: its cost is lost and it recharges instantly.
    fn fail_activation(&mut self, unit: UnitId, slot: u8) {
        let now = self.now;
        let skill_id = self
            .slot_skill(unit, slot)
            .map(|s| self.fight.skills[usize::from(s)].skill.id.get());
        let u = &mut self.units[unit.index()];
        if let Some(state) = u.bar[usize::from(slot)].as_mut() {
            state.ready_at = now;
        }
        u.action = Action::Idle;
        u.action_generation = u.action_generation.wrapping_add(1);
        let mut event = LogEvent::new(now, LogKind::SkillFailed).source(unit);
        if let Some(id) = skill_id {
            event = event.skill(id);
        }
        self.log_event(event);
        self.after_action(unit);
    }

    /// Cancels an activation: costs stay paid, no recharge, no aftercast.
    pub fn cancel(&mut self, unit: UnitId) {
        if self.units[unit.index()].activating().is_none() {
            return;
        }
        let u = &mut self.units[unit.index()];
        u.action = Action::Idle;
        u.action_generation = u.action_generation.wrapping_add(1);
        self.log_event(LogEvent::new(self.now, LogKind::SkillCancelled).source(unit));
    }

    /// Interrupts whatever the unit is doing, if the scope reaches it
    /// (T3.4.9). Returns whether anything was interrupted.
    pub fn interrupt(&mut self, unit: UnitId, by: UnitId, scope: InterruptScope) -> bool {
        if !self.units[unit.index()].alive() {
            return false;
        }
        if let Some((slot, _)) = self.units[unit.index()].activating() {
            let Some(skill) = self.slot_skill(unit, slot) else {
                return false;
            };
            let fight_skill = &self.fight.skills[usize::from(skill)];
            let kind = fight_skill.skill.kind;
            let reached = match scope {
                InterruptScope::Any | InterruptScope::Skill => true,
                InterruptScope::Spell => kind.is_a(SkillType::Spell),
                InterruptScope::Chant => kind.is_a(SkillType::Chant),
                InterruptScope::Attack => fight_skill.is_attack,
            };
            if !reached || self.prevents_interrupt(unit) {
                return false;
            }
            // The skill ends, its cost is lost, and it must recharge.
            let id = fight_skill.skill.id.get();
            let recharge = self.recharge_ms(unit, skill);
            let now = self.now;
            let u = &mut self.units[unit.index()];
            if let Some(state) = u.bar[usize::from(slot)].as_mut() {
                state.ready_at = now.plus(recharge);
            }
            u.action = Action::Idle;
            u.action_generation = u.action_generation.wrapping_add(1);
            self.log_event(
                LogEvent::new(now, LogKind::SkillInterrupted)
                    .source(by)
                    .target(unit)
                    .skill(id),
            );
            self.fire(Fired {
                event: Event::OnInterrupted,
                subject: unit,
                other: Some(by),
                skill: Some(skill),
                amount: 0.0,
            });
            self.after_action(unit);
            return true;
        }
        // Every attack can be interrupted, if a swing is under way.
        if matches!(scope, InterruptScope::Any | InterruptScope::Attack)
            && matches!(self.units[unit.index()].action, Action::Attacking { .. })
            && self.units[unit.index()].swing_hits_at.is_some()
        {
            let now = self.now;
            let u = &mut self.units[unit.index()];
            u.swing_hits_at = None;
            u.action_generation = u.action_generation.wrapping_add(1);
            u.next_swing_at = now;
            self.log_event(
                LogEvent::new(now, LogKind::SkillInterrupted)
                    .source(by)
                    .target(unit),
            );
            self.fire(Fired {
                event: Event::OnInterrupted,
                subject: unit,
                other: Some(by),
                skill: None,
                amount: 0.0,
            });
            return true;
        }
        false
    }

    /// Knocks a unit down (ENG-35). Its activation stops without counting as
    /// an interrupt and bypasses interrupt prevention, but the skill still
    /// recharges. A unit already down cannot be knocked down again until it
    /// gets up.
    pub fn knock_down(&mut self, unit: UnitId, ms: u32) {
        if !self.units[unit.index()].alive() || self.units[unit.index()].knocked_down() {
            return;
        }
        if let Some((slot, _)) = self.units[unit.index()].activating()
            && let Some(skill) = self.slot_skill(unit, slot)
        {
            let recharge = self.recharge_ms(unit, skill);
            let now = self.now;
            if let Some(state) = self.units[unit.index()].bar[usize::from(slot)].as_mut() {
                state.ready_at = now.plus(recharge);
            }
        }
        let until = self.now.plus(ms);
        let u = &mut self.units[unit.index()];
        u.action = Action::KnockedDown { until };
        u.action_generation = u.action_generation.wrapping_add(1);
        u.swing_hits_at = None;
        u.goal = None;
        u.velocity = Vec2::ZERO;
        let generation = u.action_generation;
        self.queue
            .schedule(until, EventKind::AftercastEnd { unit, generation });
        self.log_event(
            LogEvent::new(self.now, LogKind::KnockedDown)
                .target(unit)
                .amount(ms as i32),
        );
        self.fire(Fired::new(Event::OnKnockedDown, unit));
    }

    /// Disables a unit's skills, of one type or all (Disable: recharge
    /// modifiers do not touch it, and the longer of two disables wins).
    pub fn disable_skills(&mut self, unit: UnitId, which: Option<SkillType>, ms: u32) {
        let until = self.now.plus(ms);
        let fight = Arc::clone(&self.fight);
        for state in self.units[unit.index()].bar.iter_mut().flatten() {
            let kind = fight.skills[usize::from(state.skill)].skill.kind;
            if which.is_none_or(|w| kind.is_a(w)) && state.disabled_until < until {
                state.disabled_until = until;
            }
        }
    }

    /// Reverts a copied skill (Arcane Echo) and disables the original for its
    /// own recharge.
    pub(crate) fn revert_slot(&mut self, unit: UnitId, slot: u8, generation: u32) {
        let fight = Arc::clone(&self.fight);
        let now = self.now;
        let Some(state) = self.units[unit.index()].bar[usize::from(slot)].as_mut() else {
            return;
        };
        if state.revert_generation != generation {
            return;
        }
        state.skill = state.original;
        let disable = fight.skills[usize::from(state.original)]
            .skill
            .recharge
            .ms();
        state.disabled_until = state.disabled_until.max(now.plus(disable));
        state.ready_at = now;
    }

    // ------------------------------------------------------------- timing

    /// A skill's activation time for a unit, in milliseconds (ENG-12):
    ///
    /// `base × item and skill modifiers (−25%..+150%) × Fast Casting × Dazed`,
    /// halved for hard-mode foe skills over two seconds, halved again by a
    /// successful half-cast roll, and rounded to the nearest millisecond.
    /// Attack skills scale with attack speed instead.
    pub fn activation_ms(&mut self, unit: UnitId, skill: u16) -> u32 {
        let fight = Arc::clone(&self.fight);
        let fight_skill = &fight.skills[usize::from(skill)];
        let base = f64::from(fight_skill.skill.activation.ms());
        if base == 0.0 {
            return 0;
        }
        let mut time = base;
        if fight_skill.is_attack {
            time *= self.stat_multiplier(unit, Stat::AttackSpeed);
        } else {
            time *= self.stat_multiplier(unit, Stat::ActivationTime);
        }
        let kind = fight_skill.skill.kind;
        let spell = kind.is_a(SkillType::Spell);
        let signet = kind.is_a(SkillType::Signet);
        if spell || signet {
            let rank = self.rank_of(unit, Attribute::FastCasting);
            // Fast Casting leaves non-Mesmer spells and signets under 2 s alone.
            let mesmer = fight_skill.skill.profession == Some(Profession::Mesmer);
            if rank > 0 && (mesmer || base >= 2000.0) {
                if signet {
                    self.touch(17);
                }
                time *= combat::fast_casting_activation(rank);
            }
        }
        if spell && self.has_condition(unit, Condition::Dazed) {
            time *= 2.0;
        }
        // Slower casting of one skill type (Enchanter's Conundrum).
        for scoped in SkillType::ALL {
            if kind.is_a(scoped) {
                let modifiers = self.modifiers(unit, Stat::ActivationTimeOf(scoped), None);
                if !modifiers.is_empty() {
                    time *= crate::stats::combine(Stat::ActivationTimeOf(scoped), &modifiers);
                }
            }
        }
        let u = &self.units[unit.index()];
        if fight.hard_mode && u.team == Team::Foes && base > 2000.0 {
            time *= 0.5;
        }
        if spell && let Some((chance, amount)) = self.chance_mod(unit, Stat::ActivationTime, skill)
        {
            self.touch(38);
            if self.streams.unit(unit, Purpose::SkillChance).chance(chance) {
                time *= 1.0 + amount / 100.0;
            }
        }
        time.round().max(0.0) as u32
    }

    /// A skill's recharge time for a unit, in milliseconds (ENG-16):
    ///
    /// `base × item and effect reductions (to −50%) × PvE Fast Casting for
    /// Mesmer spells`, halved by a successful half-recharge roll, rounded to
    /// the nearest second (Recharge time).
    pub fn recharge_ms(&mut self, unit: UnitId, skill: u16) -> u32 {
        let fight = Arc::clone(&self.fight);
        let fight_skill = &fight.skills[usize::from(skill)];
        let base = f64::from(fight_skill.skill.recharge.ms());
        if base == 0.0 {
            return 0;
        }
        let mut time = base * self.stat_multiplier(unit, Stat::Recharge);
        let spell = fight_skill.skill.kind.is_a(SkillType::Spell);
        if spell && fight_skill.skill.profession == Some(Profession::Mesmer) {
            let rank = self.rank_of(unit, Attribute::FastCasting);
            time *= combat::fast_casting_recharge(rank);
        }
        let u = &self.units[unit.index()];
        if fight.hard_mode && u.team == Team::Foes {
            self.touch(32);
            if let Some(reduction) = fight.tunables.hard_mode_recharge_reduction {
                time *= 1.0 - reduction / 100.0;
            }
        }
        if spell && let Some((chance, amount)) = self.chance_mod(unit, Stat::Recharge, skill) {
            self.touch(38);
            if self.streams.unit(unit, Purpose::SkillChance).chance(chance) {
                time *= 1.0 + amount / 100.0;
            }
        }
        ((time / 1000.0).round() * 1000.0).max(0.0) as u32
    }
}

/// Whether using a skill on a foe draws that foe's aggro: skills with a cast
/// time do (Aggro).
fn activation_draws_aggro(skill: &crate::setup::FightSkill) -> bool {
    skill.skill.activation.ms() > 0
}
