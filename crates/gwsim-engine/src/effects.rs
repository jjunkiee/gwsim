//! Effects: hexes, enchantments, stances, conditions and the rest (WP3.6,
//! ENG-30 to ENG-32).
//!
//! Every effect lives on the unit it affects, as an [`ActiveEffect`] naming
//! where it came from. Applying, stacking, removing and expiring all happen
//! here, once, so the one-at-a-time families and the condition rules cannot
//! be bypassed by a skill that "just applies an effect".

use gwsim_data::core::Condition;
use gwsim_data::dsl::{EffectKind, StackingBehaviour};
use gwsim_data::foe::CreatureTrait;

use crate::sim::Sim;
use crate::time::{EventKind, SimTime};
use crate::unit::UnitId;

/// What an effect is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectSource {
    /// An `EffectDef` in a skill's encoding.
    Skill { skill: u16, def: u16 },
    /// One of the ten conditions.
    Condition(Condition),
    /// A handler-managed effect, keyed by its skill.
    Handler { skill: u16 },
}

/// An effect on a unit.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveEffect {
    /// Unique within the fight, so an expiry event can find it.
    pub id: u32,
    pub source: EffectSource,
    pub kind: EffectKind,
    /// Who applied it.
    pub caster: UnitId,
    /// The caster's attribute rank when it was applied. Scaled values lock
    /// in at application, as the game's do.
    pub rank: u8,
    pub applied_at: SimTime,
    /// [`None`] for maintained enchantments, which last until dropped.
    pub ends_at: Option<SimTime>,
    /// Energy pips this costs its caster while it lasts.
    pub upkeep: i8,
    /// Remaining charges per trigger, in trigger order. [`None`] is unlimited.
    pub charges: Vec<Option<u8>>,
    /// The stacking key: effects sharing it compete.
    pub key: StackKey,
    /// A slot a handler cares about (Arcane Echo's own slot).
    pub slot: Option<u8>,
}

/// What an effect competes with.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StackKey {
    Def { skill: u16, def: u16 },
    Named(String),
    Condition(Condition),
    Handler(u16),
}

/// Whether a condition can affect a unit with these traits (ENG-30).
///
/// Bleeding, Poison and Disease need flesh; spirits are immune to every
/// condition but Burning.
pub fn condition_applies(condition: Condition, traits: &[CreatureTrait]) -> bool {
    if traits.contains(&CreatureTrait::Spirit) && condition != Condition::Burning {
        return false;
    }
    match condition {
        Condition::Bleeding | Condition::Poison | Condition::Disease => {
            traits.contains(&CreatureTrait::Fleshy)
        }
        _ => true,
    }
}

/// A request to apply an effect.
#[derive(Debug, Clone)]
pub struct ApplyRequest {
    pub target: UnitId,
    pub source: EffectSource,
    pub kind: EffectKind,
    pub caster: UnitId,
    pub rank: u8,
    pub duration_ms: Option<u32>,
    pub upkeep: i8,
    pub trigger_count: usize,
    pub trigger_charges: Vec<Option<u8>>,
    pub key: StackKey,
    pub rule: StackingBehaviour,
    pub slot: Option<u8>,
}

impl Sim {
    /// Applies an effect, following its stacking rule and family.
    ///
    /// Returns the new instance's id, or [`None`] if the target is immune or
    /// a longer-lasting copy already holds.
    pub fn apply_effect(&mut self, request: ApplyRequest) -> Option<u32> {
        let target = request.target;
        if !self.units[target.index()].alive() {
            return None;
        }
        // Spirits take no hexes (ENG-30).
        if request.kind == EffectKind::Hex
            && self.units[target.index()].has_trait(CreatureTrait::Spirit)
        {
            return None;
        }
        let now = self.now;
        let ends_at = request.duration_ms.map(|ms| now.plus(ms));

        // One at a time: a new stance, preparation, glyph, form, weapon
        // spell, bundle or party bonus ends the old one of its family.
        if let Some(family) = request.kind.one_at_a_time_family() {
            let displaced: Vec<u32> = self.units[target.index()]
                .effects
                .iter()
                .filter(|e| e.kind.one_at_a_time_family() == Some(family))
                .map(|e| e.id)
                .collect();
            for id in displaced {
                self.end_effect(target, id, EndReason::Replaced);
            }
        }

        // Stacking by key.
        let existing: Vec<(u32, Option<SimTime>, UnitId)> = self.units[target.index()]
            .effects
            .iter()
            .filter(|e| e.key == request.key)
            .map(|e| (e.id, e.ends_at, e.caster))
            .collect();
        match request.rule {
            StackingBehaviour::Replace => {
                for (id, _, _) in existing {
                    self.end_effect(target, id, EndReason::Replaced);
                }
            }
            StackingBehaviour::KeepLonger => {
                for (id, old_end, _) in existing {
                    let old_lasts_longer = match (old_end, ends_at) {
                        (None, _) => true,
                        (Some(_), None) => false,
                        (Some(old), Some(new)) => old >= new,
                    };
                    if old_lasts_longer {
                        return None;
                    }
                    self.end_effect(target, id, EndReason::Replaced);
                }
            }
            StackingBehaviour::StackBySource => {
                for (id, _, caster) in existing {
                    if caster == request.caster {
                        self.end_effect(target, id, EndReason::Replaced);
                    }
                }
            }
        }

        let id = self.next_effect_id;
        self.next_effect_id += 1;
        let mut charges = request.trigger_charges;
        charges.resize(request.trigger_count, None);
        let effect = ActiveEffect {
            id,
            source: request.source,
            kind: request.kind,
            caster: request.caster,
            rank: request.rank,
            applied_at: now,
            ends_at,
            upkeep: request.upkeep,
            charges,
            key: request.key,
            slot: request.slot,
        };
        self.units[target.index()].effects.push(effect);
        if let Some(at) = ends_at {
            self.queue.schedule(
                at,
                EventKind::EffectExpiry {
                    unit: target,
                    effect: id,
                },
            );
        }
        self.log_effect(
            crate::log::LogKind::EffectApplied,
            request.caster,
            target,
            request.source,
        );
        if request.upkeep != 0 {
            self.enforce_upkeep(request.caster);
        }
        Some(id)
    }

    /// Applies a condition for a duration (ENG-30). Reapplying keeps the
    /// longer duration.
    pub fn apply_condition(
        &mut self,
        caster: UnitId,
        target: UnitId,
        condition: Condition,
        duration_ms: u32,
    ) -> bool {
        let traits = self.units[target.index()].traits.clone();
        if !condition_applies(condition, &traits) || duration_ms == 0 {
            return false;
        }
        let applied = self
            .apply_effect(ApplyRequest {
                target,
                source: EffectSource::Condition(condition),
                kind: EffectKind::Condition,
                caster,
                rank: 0,
                duration_ms: Some(duration_ms),
                upkeep: 0,
                trigger_count: 0,
                trigger_charges: Vec::new(),
                key: StackKey::Condition(condition),
                rule: StackingBehaviour::KeepLonger,
                slot: None,
            })
            .is_some();
        if applied && condition == Condition::Dazed {
            // Applying Dazed interrupts a spell in progress (ENG-18).
            if self.casting_spell(target) {
                self.interrupt(target, caster, crate::pipeline::InterruptScope::Spell);
            }
        }
        applied
    }

    /// Whether a unit carries a condition.
    pub fn has_condition(&self, unit: UnitId, condition: Condition) -> bool {
        self.units[unit.index()]
            .effects
            .iter()
            .any(|e| e.source == EffectSource::Condition(condition))
    }

    /// Whether a unit carries any effect of a kind.
    pub fn has_kind(&self, unit: UnitId, kind: EffectKind) -> bool {
        self.units[unit.index()]
            .effects
            .iter()
            .any(|e| e.kind == kind)
    }

    /// Removes up to `count` effects of a kind, most recent first, running
    /// their end actions. Returns how many were removed.
    ///
    /// Removal takes the most recently applied first (T3.6.1: the wiki
    /// describes removal skills as taking the newest hex or enchantment).
    pub fn remove_effects(&mut self, unit: UnitId, kind: EffectKind, count: usize) -> usize {
        let mut candidates: Vec<(SimTime, u32)> = self.units[unit.index()]
            .effects
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| (e.applied_at, e.id))
            .collect();
        candidates.sort_by(|a, b| b.cmp(a));
        let mut removed = 0;
        for (_, id) in candidates.into_iter().take(count) {
            self.end_effect(unit, id, EndReason::Removed);
            removed += 1;
        }
        removed
    }

    /// Removes up to `count` conditions, optionally only one kind.
    pub fn remove_conditions(
        &mut self,
        unit: UnitId,
        count: usize,
        which: Option<Condition>,
    ) -> usize {
        let mut candidates: Vec<(SimTime, u32)> = self.units[unit.index()]
            .effects
            .iter()
            .filter(|e| match (e.source, which) {
                (EffectSource::Condition(c), Some(wanted)) => c == wanted,
                (EffectSource::Condition(_), None) => true,
                _ => false,
            })
            .map(|e| (e.applied_at, e.id))
            .collect();
        candidates.sort_by(|a, b| b.cmp(a));
        let mut removed = 0;
        for (_, id) in candidates.into_iter().take(count) {
            self.end_effect(unit, id, EndReason::Removed);
            removed += 1;
        }
        removed
    }

    /// Ends one effect instance: removes it and runs its end actions.
    pub fn end_effect(&mut self, unit: UnitId, id: u32, reason: EndReason) {
        let Some(position) = self.units[unit.index()]
            .effects
            .iter()
            .position(|e| e.id == id)
        else {
            return;
        };
        let effect = self.units[unit.index()].effects.remove(position);
        let kind = match reason {
            EndReason::Expired => crate::log::LogKind::EffectEnded,
            _ => crate::log::LogKind::EffectRemoved,
        };
        self.log_effect(kind, effect.caster, unit, effect.source);
        // End actions run on expiry and on removal, not when a new copy
        // replaces the old or the bearer dies.
        if matches!(reason, EndReason::Expired | EndReason::Removed) {
            self.run_on_end(unit, &effect);
        }
        if effect.upkeep != 0 {
            self.enforce_upkeep(effect.caster);
        }
        if let EffectSource::Handler { skill } | EffectSource::Skill { skill, .. } = effect.source
            && let Some(handler) = self.fight.skills[usize::from(skill)].handler
        {
            let fight = std::sync::Arc::clone(&self.fight);
            fight
                .handlers
                .get(handler)
                .on_effect_end(self, unit, &effect, reason);
        }
        if reason == EndReason::Removed {
            let event = match effect.kind {
                EffectKind::Enchantment => Some(gwsim_data::dsl::Event::OnEnchantmentRemoved),
                EffectKind::Condition => Some(gwsim_data::dsl::Event::OnConditionRemoved),
                _ => None,
            };
            if let Some(event) = event {
                self.fire(crate::sim::Fired::new(event, unit));
            }
        }
    }

    /// Handles an expiry event, if the effect is still there.
    pub fn expire_effect(&mut self, unit: UnitId, id: u32) {
        let due = self.units[unit.index()]
            .effects
            .iter()
            .any(|e| e.id == id && e.ends_at.is_some_and(|end| end <= self.now));
        if due {
            self.end_effect(unit, id, EndReason::Expired);
        }
    }

    /// Clears the effects death clears (T3.6.1: nearly all of them).
    pub fn clear_on_death(&mut self, unit: UnitId) {
        let ids: Vec<u32> = self.units[unit.index()]
            .effects
            .iter()
            .map(|e| e.id)
            .collect();
        for id in ids {
            self.end_effect(unit, id, EndReason::Death);
        }
        // Maintained enchantments this unit was keeping up end too.
        let maintained: Vec<(UnitId, u32)> = self
            .units
            .iter()
            .flat_map(|u| {
                u.effects
                    .iter()
                    .filter(|e| e.caster == unit && e.upkeep != 0)
                    .map(move |e| (u.id, e.id))
            })
            .collect();
        for (bearer, id) in maintained {
            self.end_effect(bearer, id, EndReason::Death);
        }
    }

    /// Drops maintained enchantments while the caster's energy degeneration
    /// before the cap would be 11 or more (ENG-17). The most recent go first
    /// (Resource cost: "the most recent maintained enchantments are
    /// dropped").
    pub fn enforce_upkeep(&mut self, caster: UnitId) {
        loop {
            let (pre_cap, newest) = self.upkeep_state(caster);
            if pre_cap > -11 {
                return;
            }
            let Some((bearer, id)) = newest else { return };
            self.end_effect(bearer, id, EndReason::Dropped);
        }
    }

    /// The caster's energy pips before the cap, and its newest maintained
    /// enchantment.
    fn upkeep_state(&self, caster: UnitId) -> (i32, Option<(UnitId, u32)>) {
        let mut newest: Option<(SimTime, UnitId, u32)> = None;
        let mut upkeep = 0i32;
        for unit in &self.units {
            for effect in &unit.effects {
                if effect.caster == caster && effect.upkeep != 0 {
                    upkeep += i32::from(effect.upkeep);
                    if newest.is_none_or(|(at, _, _)| effect.applied_at >= at) {
                        newest = Some((effect.applied_at, unit.id, effect.id));
                    }
                }
            }
        }
        let pips = self.energy_pips_uncapped(caster) - upkeep;
        (pips, newest.map(|(_, bearer, id)| (bearer, id)))
    }
}

/// Why an effect ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Expired,
    /// Removed by a skill (Shatter Hex, Drain Enchantment, …).
    Removed,
    /// Replaced by a new copy or a member of its family.
    Replaced,
    /// Its bearer or caster died.
    Death,
    /// Dropped because upkeep could not be paid.
    Dropped,
    /// Cancelled by an order.
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fleshy_only_conditions_skip_the_fleshless() {
        let fleshy = [CreatureTrait::Fleshy];
        let undead = [CreatureTrait::Undead];
        assert!(condition_applies(Condition::Bleeding, &fleshy));
        assert!(!condition_applies(Condition::Bleeding, &undead));
        assert!(!condition_applies(Condition::Poison, &undead));
        assert!(condition_applies(Condition::Blind, &undead));
    }

    #[test]
    fn spirits_take_only_burning() {
        let spirit = [CreatureTrait::Spirit];
        assert!(condition_applies(Condition::Burning, &spirit));
        for condition in Condition::ALL {
            if condition != Condition::Burning {
                assert!(!condition_applies(condition, &spirit), "{condition:?}");
            }
        }
    }
}
