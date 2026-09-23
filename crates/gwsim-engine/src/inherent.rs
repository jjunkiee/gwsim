//! Inherent attribute effects the pipeline applies after a skill (T4.3.2).
//!
//! Fast Casting, Expertise, Mysticism's cost cut, Strength and Soul Reaping
//! live where they act (activation, cost, attacks, deaths). This module holds
//! the ones that act *after* a skill resolves:
//!
//! - **Divine Favor:** a Monk spell that targets an ally also heals that
//!   target for 3.2 × rank, rounded; an untargeted one heals the caster.
//! - **Leadership:** +2 energy per ally a shout or chant reaches, up to one
//!   per two ranks.

use gwsim_data::core::{Attribute, Profession, RangeBand, SkillType};
use gwsim_data::skill::TargetKind;

use crate::damage::HealKind;
use crate::sim::Sim;
use crate::unit::{ENERGY_SCALE, Target, UnitId};

/// Divine Favor's heal per rank (Divine Favor).
const DIVINE_FAVOR_PER_RANK: f64 = 3.2;
/// Leadership's energy per ally reached (Leadership).
const LEADERSHIP_PER_ALLY: i32 = 2;

impl Sim {
    /// The inherent effects that follow a successful skill.
    pub(crate) fn inherent_after_use(&mut self, unit: UnitId, target: Target, skill: u16) {
        let fight_skill = &self.fight.skills[usize::from(skill)];
        let kind = fight_skill.skill.kind;
        let monk_spell =
            fight_skill.is_spell && fight_skill.skill.profession == Some(Profession::Monk);
        let target_kind = fight_skill.skill.target;
        let shout_or_chant = kind.is_a(SkillType::Shout) || kind.is_a(SkillType::Chant);

        if monk_spell {
            let favor = self.rank_of(unit, Attribute::DivineFavor);
            if favor > 0 {
                let healed = match (target_kind, target.unit()) {
                    (
                        TargetKind::Ally | TargetKind::OtherAlly | TargetKind::AllyOrSelf,
                        Some(t),
                    ) if self.units[t.index()].team == self.units[unit.index()].team => Some(t),
                    (TargetKind::None | TargetKind::SelfOnly, _) => Some(unit),
                    _ => None,
                };
                if let Some(t) = healed {
                    let amount = (DIVINE_FAVOR_PER_RANK * f64::from(favor)).round();
                    self.heal(unit, t, amount, Some(skill), HealKind::Heal);
                }
            }
        }

        if shout_or_chant {
            let leadership = self.rank_of(unit, Attribute::Leadership);
            if leadership > 0 {
                let earshot = self.fight.core.gwinches(RangeBand::Earshot);
                let me = &self.units[unit.index()];
                let allies = self
                    .units
                    .iter()
                    .filter(|u| {
                        u.alive()
                            && u.id != unit
                            && u.team == me.team
                            && u.pos.within(me.pos, earshot)
                    })
                    .count() as i32;
                let gained = (allies * LEADERSHIP_PER_ALLY).min(i32::from(leadership) / 2);
                self.gain_energy(unit, gained * ENERGY_SCALE);
            }
        }
    }

    /// Mysticism's PvE armor: +1 core armor per rank while enchanted.
    pub fn mysticism_armor(&self, unit: UnitId) -> f64 {
        let rank = self.rank_of(unit, Attribute::Mysticism);
        if rank > 0 && self.has_kind(unit, gwsim_data::dsl::EffectKind::Enchantment) {
            f64::from(rank)
        } else {
            0.0
        }
    }
}
