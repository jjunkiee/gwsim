//! Maximum health and energy, and regeneration on the tick (T3.5.7).
//!
//! Health pips are 2 HP/s each and energy pips 1 energy per 3 s, the net of
//! each capped at ±10 after summing (Health regeneration, Energy). Natural
//! regeneration adds +1 after 5 s out of combat and +1 more every 2 s, to +7,
//! and never while the unit's other sources net to degeneration.

use gwsim_data::core::{Attribute, Condition};
use gwsim_data::dsl::Stat;

use crate::combat;
use crate::sim::Sim;
use crate::stats::combine;
use crate::time::TICK_MS;
use crate::unit::{ENERGY_SCALE, HEALTH_SCALE, UnitId, UnitKind};

/// How far Soul Reaping reaches (Soul Reaping: "about 2508 gwinches").
const SOUL_REAPING_RANGE: f32 = 2508.0;
/// Soul Reaping pays out at most this often per window.
const SOUL_REAPING_LIMIT: usize = 3;
const SOUL_REAPING_WINDOW_MS: u32 = 15_000;
/// A minion's degeneration worsens by a pip this often (Minion).
const MINION_DECAY_STEP_MS: u32 = 20_000;

impl Sim {
    /// A unit's maximum health in whole points: base, plus modifiers, less
    /// Deep Wound's 20% (at most 100), never below 1 (Health, Deep Wound).
    pub fn max_health(&self, unit: UnitId) -> i32 {
        let u = &self.units[unit.index()];
        let mut max = f64::from(u.base_max_health);
        for modifier in self.modifiers(unit, Stat::MaxHealth, None) {
            max += modifier.value;
        }
        if self.has_condition(unit, Condition::DeepWound) {
            max -= (max * 0.2).min(100.0);
        }
        max *= Self::penalty_factor(u);
        (max.round() as i32).max(1)
    }

    /// Death penalty less morale boost, as a multiplier on maximum health and
    /// energy (§10.11: −15% per death to −60%, morale to +10%).
    fn penalty_factor(u: &crate::unit::Unit) -> f64 {
        1.0 - f64::from(u.death_penalty) / 100.0 + f64::from(u.morale) / 100.0
    }

    /// A unit's maximum energy in whole points, less overcast.
    pub fn max_energy(&self, unit: UnitId) -> i32 {
        let u = &self.units[unit.index()];
        let mut max = f64::from(u.base_max_energy);
        for modifier in self.modifiers(unit, Stat::MaxEnergy, None) {
            max += modifier.value;
        }
        max *= Self::penalty_factor(u);
        let overcast = f64::from(u.overcast) / f64::from(ENERGY_SCALE);
        (max - overcast).round().max(0.0) as i32
    }

    /// Energy pips before upkeep and the cap: base plus effects.
    pub fn energy_pips_uncapped(&self, unit: UnitId) -> i32 {
        let mut pips = f64::from(self.units[unit.index()].base_energy_pips);
        for modifier in self.modifiers(unit, Stat::EnergyRegeneration, None) {
            pips += modifier.value;
        }
        pips.round() as i32
    }

    /// Energy pips after upkeep, capped at ±10.
    pub fn energy_pips(&self, unit: UnitId) -> i32 {
        let upkeep: i32 = if self.fight.any_upkeep {
            self.units
                .iter()
                .flat_map(|u| u.effects.iter())
                .filter(|e| e.caster == unit && e.upkeep != 0)
                .map(|e| i32::from(e.upkeep))
                .sum()
        } else {
            0
        };
        (self.energy_pips_uncapped(unit) - upkeep).clamp(-10, 10)
    }

    /// Health pips: effects, less condition degeneration, plus natural
    /// regeneration when nothing else nets to degeneration; capped at ±10.
    pub fn health_pips(&self, unit: UnitId) -> i32 {
        let u = &self.units[unit.index()];
        let mut pips = f64::from(u.base_health_pips);
        for modifier in self.modifiers(unit, Stat::HealthRegeneration, None) {
            pips += modifier.value;
        }
        // Degenerating conditions, in one pass over the effects.
        for effect in &u.effects {
            if let crate::effects::EffectSource::Condition(condition) = effect.source {
                pips -= match condition {
                    Condition::Bleeding => 3.0,
                    Condition::Burning => 7.0,
                    Condition::Poison | Condition::Disease => 4.0,
                    _ => 0.0,
                };
            }
        }
        let mut pips = pips.round() as i32;
        if pips >= 0 && u.natural_regeneration {
            let out_of_combat = match u.last_combat {
                Some(at) => self.now.ms().saturating_sub(at.ms()),
                // Never in combat: out of combat since the fight began.
                None => self.now.ms(),
            };
            pips += combat::natural_regeneration_pips(out_of_combat);
        }
        let mut pips = pips.clamp(-10, 10);
        // A minion decays from −1, one pip worse every 20 seconds, past the
        // displayed cap (Minion).
        if u.kind == UnitKind::Minion {
            pips -=
                1 + (self.now.ms().saturating_sub(u.born_at.ms()) / MINION_DECAY_STEP_MS) as i32;
        }
        pips
    }

    /// One tick of regeneration, degeneration and overcast recovery.
    pub(crate) fn regenerate(&mut self) {
        let ticks_per_second = (1000 / TICK_MS) as i32;
        for index in 0..self.units.len() {
            let unit = crate::unit::UnitId(index as u16);
            if !self.units[index].alive() {
                continue;
            }
            // 2 HP per second per pip, in milli-HP per tick, plus any flat
            // health per second ("Incoming!").
            let per_second: f64 = self
                .modifiers(unit, Stat::HealthPerSecond, None)
                .iter()
                .map(|m| m.value)
                .sum();
            let health = self.health_pips(unit) * 2 * HEALTH_SCALE / ticks_per_second
                + (per_second * f64::from(HEALTH_SCALE) / f64::from(ticks_per_second)).round()
                    as i32;
            // 1 energy per 3 seconds per pip, in units per tick.
            let energy = self.energy_pips(unit) * ENERGY_SCALE / (3 * ticks_per_second);
            let max_health = self.max_health(unit) * HEALTH_SCALE;
            let max_energy = self.max_energy(unit) * ENERGY_SCALE;
            let u = &mut self.units[index];
            if health != 0 {
                u.health = (u.health + health).min(max_health);
            } else if u.health > max_health {
                u.health = max_health;
            }
            if energy > 0 {
                if u.energy < max_energy {
                    u.energy = (u.energy + energy).min(max_energy);
                }
            } else if energy < 0 {
                u.energy = (u.energy + energy).max(0);
            }
            if u.energy > max_energy {
                u.energy = max_energy;
            }
            // Overcast recovers at 1 maximum energy per 3 seconds.
            if u.overcast > 0 {
                u.overcast = (u.overcast - ENERGY_SCALE / (3 * ticks_per_second)).max(0);
            }
            if u.health <= 0 {
                self.kill(unit, None);
            }
        }
    }

    /// Soul Reaping: +1 energy per rank when a non-spirit creature dies
    /// nearby, at most three times per 15 seconds, and only below maximum
    /// energy (Soul Reaping).
    pub(crate) fn soul_reaping(&mut self, dead: UnitId) {
        if self.units[dead.index()].kind == UnitKind::Spirit {
            return;
        }
        let position = self.units[dead.index()].pos;
        let now = self.now;
        for index in 0..self.units.len() {
            let reaper = crate::unit::UnitId(index as u16);
            let rank = self.units[index].base_ranks[Attribute::SoulReaping.index()];
            if rank == 0
                || !self.units[index].alive()
                || !self.units[index].pos.within(position, SOUL_REAPING_RANGE)
            {
                continue;
            }
            let rank = self.rank_of(reaper, Attribute::SoulReaping);
            if self.units[index].energy >= self.max_energy(reaper) * ENERGY_SCALE {
                continue;
            }
            let recent = &mut self.units[index].soul_reaping;
            recent.retain(|at| now.ms().saturating_sub(at.ms()) < SOUL_REAPING_WINDOW_MS);
            if recent.len() >= SOUL_REAPING_LIMIT {
                continue;
            }
            recent.push(now);
            self.gain_energy(reaper, i32::from(rank) * ENERGY_SCALE);
        }
    }

    /// The combined multiplier for a percentage stat on a unit.
    pub fn stat_multiplier(&self, unit: UnitId, stat: Stat) -> f64 {
        combine(stat, &self.modifiers(unit, stat, None))
    }
}
