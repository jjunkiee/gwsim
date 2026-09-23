//! Choosing a skill and a target: the evaluator foes and heroes share
//! (T4.4.5, T4.5.4).
//!
//! Every usable skill on the bar is scored against each target it could
//! reach, from its [`SkillProfile`](super::profile::SkillProfile). A skill
//! that would be wasted scores nothing: a heal on a healthy ally, a removal
//! with nothing to remove, an effect already up (heroes never reapply one,
//! Hero behavior), a conditional skill whose condition fails. The highest
//! score wins, the first slot on a tie. The scores encode the heroes'
//! documented order of concerns: raise the dead, save the dying, stop a cast,
//! cleanse, protect, keep effects up, then deal damage.

use gwsim_data::dsl::EffectKind;
use gwsim_data::skill::TargetKind;

use crate::effects::EffectSource;
use crate::sim::Sim;
use crate::unit::{Target, Team, UnitId, UnitKind};

/// Scores for each concern, highest first.
const RAISE: f64 = 100.0;
const INTERRUPT: f64 = 55.0;
const CLEANSE_HEX: f64 = 60.0;
const HEAL_BASE: f64 = 50.0;
const BATTERY: f64 = 45.0;
const TEARDOWN: f64 = 45.0;
const SPIRIT: f64 = 40.0;
const SUMMON: f64 = 38.0;
const MAINTAIN: f64 = 35.0;
const ENERGY: f64 = 35.0;
const BUNDLE: f64 = 33.0;
const HEX: f64 = 30.0;
const PARTY_BUFF: f64 = 30.0;
const CREATURES: f64 = 30.0;
const DAMAGE: f64 = 20.0;
/// A heal is not worth casting above this share of maximum health.
const HEAL_BELOW: f64 = 0.8;
/// A battery goes on an ally at or below this share of energy (Hero behavior).
const BATTERY_BELOW: f64 = 0.5;
/// Energy skills are for a user below this share of energy.
const ENERGY_BELOW: f64 = 0.7;
/// How far past a skill's range a controller will walk to use it.
const REACH_MARGIN: f32 = 200.0;

/// What a controller knows when it chooses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Situation {
    /// The foe it is concentrating on.
    pub focus: Option<UnitId>,
    /// Whether it is fighting: shouts, spirits and damage wait for this.
    pub in_combat: bool,
    /// Offensive skills are off (a hero on Avoid Combat).
    pub no_offence: bool,
    /// Foe skills must stay on the focus when it can take them (a hero
    /// locked on a target, AI-H1).
    pub locked: bool,
    /// Only interrupts are wanted (a hero inside its reaction delay, A-011).
    pub interrupts_only: bool,
}

impl Sim {
    /// The best skill and target for a unit now, if any is worth using.
    pub fn best_skill(&self, unit: UnitId, situation: &Situation) -> Option<(u8, Target)> {
        let mut best: Option<(f64, u8, Target)> = None;
        for slot in 0..self.units[unit.index()].bar.len() as u8 {
            let Some(skill) = self.slot_skill(unit, slot) else {
                continue;
            };
            // Skills the tactics plan disables are never used on the AI's
            // own initiative (AI-H9).
            if self.units[unit.index()].disabled_slots & (1 << slot) != 0 {
                continue;
            }
            // What does not depend on the target first: recharge, energy,
            // adrenaline, a busy caster.
            if !self.ready(unit, slot) {
                continue;
            }
            for target in self.candidates(unit, skill, situation) {
                if self.can_use(unit, slot, target).is_err() {
                    continue;
                }
                let Some(score) = self.score(unit, skill, target, situation) else {
                    continue;
                };
                if best.is_none_or(|(s, _, _)| score > s) {
                    best = Some((score, slot, target));
                }
            }
        }
        best.map(|(_, slot, target)| (slot, target))
    }

    /// The targets a skill could be used on.
    fn candidates(&self, unit: UnitId, skill: u16, situation: &Situation) -> Vec<Target> {
        let fight_skill = &self.fight.skills[usize::from(skill)];
        let me = &self.units[unit.index()];
        let reach = self.skill_range(unit, skill).unwrap_or(
            self.fight
                .core
                .gwinches(gwsim_data::core::RangeBand::Casting),
        ) + REACH_MARGIN;
        let within = |id: UnitId| self.units[id.index()].pos.within(me.pos, reach);
        match fight_skill.skill.target {
            TargetKind::Foe => {
                let mut foes: Vec<UnitId> = self
                    .units
                    .iter()
                    .filter(|u| u.alive() && u.team != me.team && within(u.id))
                    .map(|u| u.id)
                    .collect();
                if let Some(focus) = situation.focus
                    && let Some(position) = foes.iter().position(|f| *f == focus)
                {
                    foes.swap(0, position);
                    // A locked hero keeps foe skills on its focus, except
                    // hexes it can spread once the focus has one.
                    if situation.locked && fight_skill.profile.target_effect.is_none() {
                        foes.truncate(1);
                    }
                }
                foes.into_iter().map(Target::Unit).collect()
            }
            TargetKind::Ally | TargetKind::AllyOrSelf | TargetKind::OtherAlly => {
                let other = fight_skill.skill.target == TargetKind::OtherAlly;
                self.units
                    .iter()
                    .filter(|u| {
                        u.alive()
                            && u.team == me.team
                            && !u.kind.is_summoned()
                            && !(other && u.id == unit)
                            && within(u.id)
                    })
                    .map(|u| Target::Unit(u.id))
                    .collect()
            }
            TargetKind::Corpse => self
                .units
                .iter()
                .filter(|u| {
                    !u.alive() && u.team == me.team && !u.kind.is_summoned() && within(u.id)
                })
                .map(|u| Target::Unit(u.id))
                .collect(),
            TargetKind::Spirit | TargetKind::Minion => Vec::new(),
            TargetKind::SelfOnly | TargetKind::None | TargetKind::Location => {
                vec![Target::Unit(unit)]
            }
        }
    }

    /// How much a skill is worth on a target now, or [`None`] if it would be
    /// wasted.
    fn score(
        &self,
        unit: UnitId,
        skill: u16,
        target: Target,
        situation: &Situation,
    ) -> Option<f64> {
        let fight_skill = &self.fight.skills[usize::from(skill)];
        let profile = &fight_skill.profile;
        let t = target.unit().unwrap_or(unit);
        let hostile_target = self.units[t.index()].team != self.units[unit.index()].team;
        let mut score: f64 = 0.0;

        if situation.no_offence && (profile.damages || profile.interrupts || hostile_target) {
            return None;
        }
        if situation.interrupts_only && !(profile.interrupts && profile.requirement.is_some()) {
            return None;
        }
        if let Some(requirement) = &profile.requirement
            && !self.filter_passes(requirement, t, unit, None)
        {
            return None;
        }
        if profile.resurrects {
            return (!self.units[t.index()].alive()).then_some(RAISE);
        }
        if let Some(kind) = profile.removes {
            if !self.has_kind(t, kind) {
                return None;
            }
            score = score.max(if kind == EffectKind::Hex {
                CLEANSE_HEX
            } else {
                TEARDOWN
            });
        }
        if profile.heals_target && !hostile_target {
            let health = self.health_fraction(t);
            if health > HEAL_BELOW {
                return None;
            }
            score = score.max(HEAL_BASE + (1.0 - health) * 100.0);
        }
        if profile.battery {
            let ally = &self.units[t.index()];
            let energy = f64::from(ally.energy_points()) / f64::from(self.max_energy(t).max(1));
            if t == unit || energy > BATTERY_BELOW || !ally.holds_caster_weapon() {
                return None;
            }
            score = score.max(BATTERY);
        }
        if let Some(def) = profile.target_effect {
            if self.bears(t, skill, def) {
                return None;
            }
            score = score.max(if hostile_target { HEX } else { MAINTAIN });
        }
        if let Some(def) = profile.self_effect {
            if self.bears(unit, skill, def) || !situation.in_combat {
                return None;
            }
            score = score.max(MAINTAIN);
        }
        if let Some(def) = profile.party_effect {
            if self.bears(unit, skill, def) {
                return None;
            }
            // Speed boosts go out when the party moves; other shouts in
            // combat (Hero behavior: shouts).
            let wanted = if profile.speed_boost {
                self.units[unit.index()].moving()
            } else {
                situation.in_combat
            };
            if !wanted {
                return None;
            }
            score = score.max(PARTY_BUFF);
        }
        if let Some(spirit) = &profile.spirit {
            // Passive spirits only in combat, and never over a live one of
            // the same type (Hero behavior: binding rituals).
            let team = self.units[unit.index()].team;
            let standing = self.units.iter().any(|u| {
                u.alive()
                    && u.kind == UnitKind::Spirit
                    && u.team == team
                    && u.creature_type.as_deref() == Some(spirit.as_str())
            });
            if standing || !situation.in_combat {
                return None;
            }
            score = score.max(SPIRIT);
        }
        if profile.summons {
            score = score.max(SUMMON);
        }
        if profile.needs_creatures {
            let owns = self
                .units
                .iter()
                .any(|u| u.alive() && u.master == Some(unit) && u.kind.is_summoned());
            if !owns {
                return None;
            }
            score = score.max(CREATURES);
        }
        if profile.bundle {
            let holding = self.units[unit.index()]
                .effects
                .iter()
                .any(|e| e.kind == EffectKind::Bundle);
            if holding {
                return None;
            }
            score = score.max(BUNDLE);
        }
        if profile.interrupts && profile.requirement.is_some() {
            score = score.max(INTERRUPT);
        }
        // Energy is the point only of a skill that does nothing else: a
        // heal that refunds energy (Zealous Benediction) is still a heal.
        if profile.gains_energy
            && !profile.damages
            && !profile.heals_target
            && profile.removes.is_none()
        {
            let me = &self.units[unit.index()];
            let energy = f64::from(me.energy_points()) / f64::from(self.max_energy(unit).max(1));
            if energy > ENERGY_BELOW {
                return None;
            }
            score = score.max(ENERGY);
        }
        if profile.damages {
            if !situation.in_combat {
                return None;
            }
            let bonus = if Some(t) == situation.focus { 5.0 } else { 0.0 };
            score = score.max(DAMAGE + bonus);
        }
        if profile.heals_party {
            score = score.max(PARTY_BUFF);
        }
        if score == 0.0 {
            // Nothing the profile recognises: use it in combat, lowest.
            if !situation.in_combat {
                return None;
            }
            score = 10.0;
        }
        // A skill's own hints raise or lower it.
        if let Some(hints) = fight_skill
            .skill
            .encoding
            .as_ref()
            .and_then(|e| e.ai.as_ref())
        {
            if hints
                .never_when
                .iter()
                .any(|f| self.filter_passes(f, t, unit, None))
            {
                return None;
            }
            if !hints
                .use_when
                .iter()
                .all(|f| self.filter_passes(f, t, unit, None))
            {
                return None;
            }
            score += f64::from(hints.priority) * 10.0;
        }
        Some(score)
    }

    /// Whether a slot could be used now, whatever the target.
    fn ready(&self, unit: UnitId, slot: u8) -> bool {
        match self.can_use(unit, slot, Target::Unit(unit)) {
            Ok(()) => true,
            Err(crate::pipeline::Invalid::BadTarget(_)) => true,
            Err(_) => false,
        }
    }

    /// Whether a unit bears an effect from a skill: a given definition, or
    /// (for handlers, `u16::MAX`) any effect of the skill.
    pub fn bears(&self, unit: UnitId, skill: u16, def: u16) -> bool {
        self.units[unit.index()]
            .effects
            .iter()
            .any(|e| match e.source {
                EffectSource::Skill { skill: s, def: d } => {
                    s == skill && (def == u16::MAX || d == def)
                }
                EffectSource::Handler { skill: s } => s == skill,
                EffectSource::Condition(_) => false,
            })
    }

    /// Whether any living member of a team is within a radius of a unit.
    pub fn team_near(&self, unit: UnitId, team: Team, radius: f32) -> bool {
        let me = self.units[unit.index()].pos;
        self.units
            .iter()
            .any(|u| u.alive() && u.team == team && u.pos.within(me, radius))
    }
}
