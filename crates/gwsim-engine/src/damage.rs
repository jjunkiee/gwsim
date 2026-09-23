//! Applying damage, healing, energy and death to units (T3.5.4 to T3.5.8).
//!
//! The formulas are in [`crate::combat`]; this module is where their results
//! meet unit state, in the wiki's order (A-022): armor first, then damage
//! multipliers, then the reductions of the target's effects in the order they
//! were applied, then the hit lands. Mitigation is credited to whoever's
//! effect did the mitigating (§14.2).

use std::sync::Arc;

use gwsim_data::core::SkillType;
use gwsim_data::core::{ArmorSlot, Condition, DamageType};
use gwsim_data::dsl::{DamageSource, Event, ModCategory, Stat};
use gwsim_data::foe::CreatureTrait;

use crate::combat;
use crate::log::{LogEvent, LogKind};
use crate::pipeline::InterruptScope;
use crate::rng::Purpose;
use crate::sim::{Fired, Sim};
use crate::unit::{Action, ENERGY_SCALE, HEALTH_SCALE, Team, UnitId};

/// How a damage packet was made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageInfo {
    pub kind: Option<DamageType>,
    /// Armor-ignoring damage ignores armor and the attacker's level alike
    /// (Armor-ignoring damage).
    pub armor_ignoring: bool,
    pub skill: Option<u16>,
    /// Whether this is a weapon attack's packet.
    pub attack: bool,
    pub strike_level: f64,
    /// Armor penetration, as a fraction.
    pub penetration: f64,
    pub critical: bool,
    /// The armor piece struck, if already chosen.
    pub piece: Option<ArmorSlot>,
}

/// One damage reduction from an effect (the four shapes of
/// `ReduceIncomingDamage`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reduction {
    pub flat: Option<f64>,
    pub percent: Option<f64>,
    /// A ceiling on one packet, as a percentage of maximum health (Shelter).
    pub cap: Option<f64>,
    pub only_from: Option<DamageSource>,
    /// The most it prevents from one packet (Reversal of Fortune).
    pub limit: Option<f64>,
    /// What it prevents heals the bearer instead.
    pub heals: bool,
    /// Health the granting creature loses each time it prevents damage
    /// (Shelter, Union).
    pub cost_to_source: Option<f64>,
    /// Who granted it: an effect's caster, or an aura's spirit.
    pub caster: UnitId,
    pub skill: u16,
}

/// Healing or health gain (Heal): modifiers apply only to healing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealKind {
    Heal,
    HealthGain,
}

impl Sim {
    /// A unit's armor against a damage type on one piece, after the four-step
    /// armor calculation.
    pub fn armor_against(
        &self,
        unit: UnitId,
        damage: DamageType,
        piece: ArmorSlot,
        penetration: f64,
    ) -> f64 {
        let u = &self.units[unit.index()];
        let mut core =
            f64::from(u.armor[piece.index()].against(damage)) + self.mysticism_armor(unit);
        let mut bonuses = Vec::new();
        let mut special = 0.0;
        for modifier in self.modifiers(unit, Stat::Armor, Some(piece)) {
            match modifier.category {
                ModCategory::Core => core += modifier.value,
                ModCategory::Special => special += modifier.value,
                _ => bonuses.push(modifier.value),
            }
        }
        combat::armor_level(core, &bonuses, penetration, special)
    }

    /// Deals one damage packet. Returns the damage done, in whole points.
    pub fn deal_damage(
        &mut self,
        source: UnitId,
        target: UnitId,
        base: f64,
        info: DamageInfo,
    ) -> i32 {
        if !self.units[target.index()].alive() || base <= 0.0 {
            return 0;
        }
        let mut amount = if info.armor_ignoring {
            base
        } else {
            let damage_type = info.kind.unwrap_or(DamageType::Chaos);
            let piece = info.piece.unwrap_or_else(|| {
                self.touch(if info.attack { 14 } else { 39 });
                let roll = self.streams.unit(source, Purpose::Hits).unit_f64();
                combat::hit_location(roll)
            });
            self.touch(23);
            let armor = self.armor_against(target, damage_type, piece, info.penetration);
            let strike = if info.critical {
                info.strike_level + 20.0
            } else {
                info.strike_level
            };
            combat::damage_packet(base, strike, armor)
        };

        // Multipliers on the dealer and the target (uncapped, ENG-34).
        let dealt = crate::stats::combine(
            Stat::DamageDealt,
            &self.modifiers(source, Stat::DamageDealt, None),
        );
        let taken = crate::stats::combine(
            Stat::DamageTaken,
            &self.modifiers(target, Stat::DamageTaken, None),
        );
        amount *= dealt * taken;

        // The target's reductions, in the order their effects were applied
        // (A-022: the wiki's listed order among M1's effects is application
        // order in practice, since each unit carries few).
        self.touch(22);
        let max_health = f64::from(self.max_health(target));
        let source_has_condition = self.units[source.index()]
            .effects
            .iter()
            .any(|e| e.kind == gwsim_data::dsl::EffectKind::Condition);
        let from_spell = info.skill.is_some_and(|s| {
            self.fight.skills[usize::from(s)]
                .skill
                .kind
                .is_a(SkillType::Spell)
        });
        let mut converted: Vec<(UnitId, f64, u16)> = Vec::new();
        let mut spirit_costs: Vec<(UnitId, f64)> = Vec::new();
        for reduction in self.damage_reductions(target) {
            let applies = match reduction.only_from {
                None => true,
                Some(DamageSource::Spells) => from_spell,
                Some(DamageSource::Attacks) => info.attack,
                Some(DamageSource::FoesWithConditions) => source_has_condition,
            };
            if !applies {
                continue;
            }
            let before = amount;
            if let Some(cap) = reduction.cap {
                amount = amount.min(max_health * cap / 100.0);
            }
            if let Some(percent) = reduction.percent {
                amount *= 1.0 - percent / 100.0;
            }
            if let Some(flat) = reduction.flat {
                amount = (amount - flat).max(0.0);
            }
            if let Some(limit) = reduction.limit {
                amount = amount.max(before - limit);
            }
            let saved = before - amount;
            if saved > 0.0 {
                if reduction.heals {
                    converted.push((reduction.caster, saved, reduction.skill));
                }
                if let Some(cost) = reduction.cost_to_source {
                    spirit_costs.push((reduction.caster, cost));
                }
            }
            let prevented = combat::round_points(before - amount);
            if prevented > 0 {
                self.stats.skills[usize::from(reduction.skill)].mitigation += i64::from(prevented);
            }
        }

        let damage = combat::round_points(amount).max(0);
        self.apply_health_change(target, -damage * HEALTH_SCALE);
        // Prevented damage that heals instead, and what preventing costs the
        // spirits that did it.
        for (healer, saved, skill) in converted {
            self.heal(healer, target, saved, Some(skill), HealKind::HealthGain);
        }
        for (spirit, cost) in spirit_costs {
            self.health_loss(spirit, cost, Some(source));
        }
        self.mark_combat(source);
        self.mark_combat(target);

        if let Some(slot) = self.units[source.index()].slot_index
            && self.units[source.index()].team == Team::Party
        {
            self.stats.slots[slot].damage_dealt += i64::from(damage);
        }
        if let Some(slot) = self.units[target.index()].slot_index {
            self.stats.slots[slot].damage_taken += i64::from(damage);
        }
        // Damage a skill does to its own side (a spirit paying for a block)
        // is a cost, not damage dealt.
        let hostile = self.units[source.index()].team != self.units[target.index()].team;
        if let Some(skill) = info.skill
            && hostile
        {
            self.stats.skills[usize::from(skill)].damage += i64::from(damage);
        }
        if self.logging() {
            let mut event = LogEvent::new(self.now, LogKind::Damage)
                .source(source)
                .target(target)
                .amount(damage);
            if let Some(skill) = info.skill {
                event = event.skill(self.fight.skills[usize::from(skill)].skill.id.get());
            }
            if let Some(kind) = info.kind {
                event = event.detail(&format!(
                    "{kind:?}{}",
                    if info.armor_ignoring && !info.attack {
                        ", armor-ignoring"
                    } else {
                        ""
                    }
                ));
            }
            self.log_event(event);
        }

        // Adrenaline from taking damage: one unit per 1% of maximum health
        // lost, counted before reductions (Adrenaline).
        if max_health > 0.0 {
            let units = (base / max_health * 100.0).floor() as i32;
            if units > 0 {
                self.gain_adrenaline(target, units);
            }
        }

        // Damage interrupts a Dazed spell and, from an attack, anything
        // easily interrupted (Interrupt).
        if damage > 0 && self.units[target.index()].alive() {
            let dazed_spell =
                self.casting_spell(target) && self.has_condition(target, Condition::Dazed);
            let easy = info.attack
                && self.units[target.index()]
                    .activating()
                    .and_then(|(slot, _)| self.slot_skill(target, slot))
                    .is_some_and(|s| {
                        self.fight.skills[usize::from(s)]
                            .skill
                            .flags
                            .easily_interrupted
                    });
            if dazed_spell || easy {
                self.interrupt(target, source, InterruptScope::Any);
            }
        }

        self.fire(Fired {
            event: Event::OnDamageTaken,
            subject: target,
            other: Some(source),
            skill: info.skill,
            amount: f64::from(damage),
        });
        self.fire(Fired {
            event: Event::OnDamageDealt,
            subject: source,
            other: Some(target),
            skill: info.skill,
            amount: f64::from(damage),
        });

        if self.units[target.index()].alive() && self.units[target.index()].health <= 0 {
            self.kill(target, Some(source));
        }
        damage
    }

    /// Direct health loss: not damage, so no armor or reduction (Heal).
    /// Returns the health lost.
    pub fn health_loss(&mut self, target: UnitId, amount: f64, source: Option<UnitId>) -> f64 {
        if !self.units[target.index()].alive() {
            return 0.0;
        }
        let points = combat::round_points(amount).max(0);
        self.apply_health_change(target, -points * HEALTH_SCALE);
        self.mark_combat(target);
        if self.units[target.index()].health <= 0 {
            self.kill(target, source);
        }
        f64::from(points)
    }

    /// A sacrifice of a percentage of maximum health, after a successful
    /// activation (ENG-13). It can kill.
    pub fn sacrifice(&mut self, unit: UnitId, percent: f64) {
        let max = f64::from(self.max_health(unit));
        self.health_loss(unit, max * percent / 100.0, None);
    }

    /// Heals or grants health. Healing takes the target's healing modifiers,
    /// capped at −40% (ENG-33); health gain takes none. Overhealing is
    /// counted separately (§14.2).
    pub fn heal(
        &mut self,
        source: UnitId,
        target: UnitId,
        amount: f64,
        skill: Option<u16>,
        kind: HealKind,
    ) -> f64 {
        if !self.units[target.index()].alive() || amount <= 0.0 {
            return 0.0;
        }
        let amount = match kind {
            HealKind::Heal => {
                amount
                    * crate::stats::combine(
                        Stat::HealingReceived,
                        &self.modifiers(target, Stat::HealingReceived, None),
                    )
            }
            HealKind::HealthGain => amount,
        };
        let points = combat::round_points(amount).max(0);
        let max = self.max_health(target) * HEALTH_SCALE;
        let before = self.units[target.index()].health;
        let after = (before + points * HEALTH_SCALE).min(max);
        self.units[target.index()].health = after;
        let gained = (after - before) / HEALTH_SCALE;
        let over = points - gained;
        if let Some(slot) = self.units[source.index()].slot_index {
            self.stats.slots[slot].healing_done += i64::from(gained);
            self.stats.slots[slot].overhealing += i64::from(over);
        }
        if let Some(slot) = self.units[target.index()].slot_index {
            self.stats.slots[slot].healing_received += i64::from(gained);
        }
        if let Some(skill) = skill {
            self.stats.skills[usize::from(skill)].healing += i64::from(gained);
        }
        if self.logging() {
            self.log_event(
                LogEvent::new(self.now, LogKind::Heal)
                    .source(source)
                    .target(target)
                    .amount(points),
            );
        }
        if kind == HealKind::Heal {
            self.fire(Fired {
                event: Event::OnHeal,
                subject: target,
                other: Some(source),
                skill,
                amount: f64::from(points),
            });
        }
        f64::from(gained)
    }

    /// Adds energy in 1/3000 units, up to the maximum.
    pub fn gain_energy(&mut self, unit: UnitId, units: i32) {
        if !self.units[unit.index()].alive() || units <= 0 {
            return;
        }
        let max = self.max_energy(unit) * ENERGY_SCALE;
        let u = &mut self.units[unit.index()];
        let before = u.energy;
        u.energy = (u.energy + units).min(max.max(u.energy));
        let gained = u.energy - before;
        if let Some(slot) = u.slot_index {
            self.stats.slots[slot].energy_gained += i64::from(gained / ENERGY_SCALE);
        }
        self.fire(Fired {
            event: Event::OnEnergyChanged,
            subject: unit,
            other: None,
            skill: None,
            amount: f64::from(gained) / f64::from(ENERGY_SCALE),
        });
    }

    /// Takes whole points of energy, never below zero. Returns what was taken
    /// (Energy Surge's damage counts exactly this).
    pub fn lose_energy(&mut self, unit: UnitId, points: i32) -> i32 {
        if !self.units[unit.index()].alive() || points <= 0 {
            return 0;
        }
        let u = &mut self.units[unit.index()];
        let available = u.energy.max(0) / ENERGY_SCALE;
        let lost = points.min(available);
        u.energy -= lost * ENERGY_SCALE;
        if lost > 0 {
            self.mark_combat(unit);
            self.fire(Fired {
                event: Event::OnEnergyChanged,
                subject: unit,
                other: None,
                skill: None,
                amount: -f64::from(lost),
            });
        }
        lost
    }

    /// Adds (or removes) adrenaline to every adrenal skill that can gain it.
    /// Recharging and disabled skills gain none (Adrenaline).
    pub fn gain_adrenaline(&mut self, unit: UnitId, units: i32) {
        let rate = if units > 0 {
            crate::stats::combine(
                Stat::AdrenalineRate,
                &self.modifiers(unit, Stat::AdrenalineRate, None),
            )
        } else {
            1.0
        };
        let now = self.now;
        let fight = Arc::clone(&self.fight);
        for slot in self.units[unit.index()].bar.iter_mut().flatten() {
            let cost = i32::from(fight.skills[usize::from(slot.skill)].skill.cost.adrenaline) * 25;
            if cost == 0 || slot.ready_at > now || slot.disabled_until > now {
                continue;
            }
            let added = (f64::from(units) * rate).round() as i32;
            slot.adrenaline = (slot.adrenaline + added).clamp(0, cost);
        }
    }

    /// Changes health, keeping it at or below the maximum.
    pub fn apply_health_change(&mut self, unit: UnitId, milli: i32) {
        let max = self.max_health(unit) * HEALTH_SCALE;
        let u = &mut self.units[unit.index()];
        u.health = (u.health + milli).min(max);
    }

    /// Marks a unit as in active combat now (for natural regeneration).
    pub fn mark_combat(&mut self, unit: UnitId) {
        self.units[unit.index()].last_combat = Some(self.now);
        if self.engaged_at.is_none() {
            self.engaged_at = Some(self.now);
        }
    }

    /// Kills a unit (T3.5.8). Its activation stops (not an interrupt), most
    /// effects clear, a fleshy body leaves a corpse, and the kill may grant
    /// experience (A-036).
    pub fn kill(&mut self, unit: UnitId, killer: Option<UnitId>) {
        if !self.units[unit.index()].alive() {
            return;
        }
        let now = self.now;
        {
            let u = &mut self.units[unit.index()];
            u.action = Action::Dead;
            u.action_generation = u.action_generation.wrapping_add(1);
            u.health = 0;
            u.energy = 0;
            u.goal = None;
            u.velocity = crate::geom::Vec2::ZERO;
            u.attack_target = None;
            u.dead_at = Some(now);
            u.corpse_available = u.has_trait(CreatureTrait::Fleshy);
            for slot in u.bar.iter_mut().flatten() {
                slot.adrenaline = 0;
            }
        }
        // On-death triggers read the effects the unit still bears (Putrid
        // Bile, Blood Bond), so they fire before death clears them.
        self.fire(Fired {
            event: Event::OnDeath,
            subject: unit,
            other: killer,
            skill: None,
            amount: 0.0,
        });
        self.clear_on_death(unit);
        self.creature_died(unit);
        let u = &self.units[unit.index()];
        let (slot_index, foe_index) = (u.slot_index, u.foe_index);
        let party_member = u.team == Team::Party && !u.kind.is_summoned();
        if let Some(slot) = slot_index {
            self.stats.slots[slot].deaths += 1;
        }
        if party_member {
            // Death penalty: 15% per death, to 60% (§10.11).
            let u = &mut self.units[unit.index()];
            u.death_penalty = (u.death_penalty + 15).min(60);
            if self.fight.dhuums_covenant {
                self.covenant_broken = true;
            }
        }
        if let Some(foe) = foe_index {
            let engaged = self.engaged_at.unwrap_or(crate::time::SimTime::ZERO);
            self.stats.foe_ttk_ms[foe] = Some(now.ms().saturating_sub(engaged.ms()));
        }
        self.log_event({
            let mut event = LogEvent::new(now, LogKind::Death).target(unit);
            if let Some(killer) = killer {
                event = event.source(killer);
            }
            event
        });

        if let Some(killer) = killer {
            self.fire(Fired {
                event: Event::OnKill,
                subject: killer,
                other: Some(unit),
                skill: None,
                amount: 0.0,
            });
        }

        let dead = &self.units[unit.index()];
        if dead.gives_experience && dead.team == Team::Foes {
            let position = dead.pos;
            self.touch(36);
            let range = self.fight.tunables.experience_range;
            let earners: Vec<UnitId> = self
                .units
                .iter()
                .filter(|u| u.team == Team::Party && u.alive() && !u.kind.is_summoned())
                .filter(|u| u.pos.within(position, range))
                .map(|u| u.id)
                .collect();
            for earner in earners {
                self.fire(Fired {
                    event: Event::OnExperienceKill,
                    subject: earner,
                    other: Some(unit),
                    skill: None,
                    amount: 0.0,
                });
            }
        }
        self.soul_reaping(unit);
        self.check_outcome();
    }
}
