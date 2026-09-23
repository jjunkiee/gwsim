//! The M1 handlers beyond the two M0 needs (WP4.1, T1.5.1): the skills the
//! DSL cannot say.
//!
//! - **Soul Twisting** changes the cost and recharge of a whole skill type
//!   and counts its uses.
//! - **Master of Magic** *sets* four attributes rather than adding to them,
//!   and returns energy on elemental spells.
//! - **Resurrection Chant** raises an ally with up to the caster's *current*
//!   health, a quantity of the caster that the DSL's `Resurrect` (a share of
//!   the target's maximum) cannot name.

use std::collections::BTreeMap;

use gwsim_data::core::{Attribute, SkillType};
use gwsim_data::dsl::{EffectKind, Event, HandlerDescribe, StackingBehaviour, Value};

use super::{HandlerRegistry, SkillHandler};
use crate::effects::{ActiveEffect, ApplyRequest, EffectSource, EndReason, StackKey};
use crate::exec::ExecCtx;
use crate::sim::{Fired, Sim};
use crate::unit::{ENERGY_SCALE, UnitId};

/// Registers the M1 handlers.
pub fn register(registry: &mut HandlerRegistry) {
    registry.register(Box::new(SoulTwisting));
    registry.register(Box::new(MasterOfMagic));
    registry.register(Box::new(ResurrectionChant));
}

/// A parameter evaluated in a context, or a default.
fn param(sim: &Sim, ctx: &ExecCtx, skill: u16, name: &str, default: Value) -> f64 {
    let params = sim.handler_params(skill);
    let value = params.get(name).cloned().unwrap_or(default);
    sim.eval_value(&value, ctx)
}

/// Applies a handler effect to the caster.
fn apply_self(
    sim: &mut Sim,
    ctx: &ExecCtx,
    skill: u16,
    kind: EffectKind,
    seconds: f64,
    charges: Option<u8>,
) {
    sim.apply_effect(ApplyRequest {
        target: ctx.caster,
        source: EffectSource::Handler { skill },
        kind,
        caster: ctx.caster,
        rank: ctx.rank,
        duration_ms: Some((seconds * 1000.0).round().max(0.0) as u32),
        upkeep: 0,
        trigger_count: usize::from(charges.is_some()),
        trigger_charges: charges.map(|c| vec![Some(c)]).unwrap_or_default(),
        key: StackKey::Handler(skill),
        rule: StackingBehaviour::Replace,
        slot: ctx.slot,
    });
}

// ------------------------------------------------------------- Soul Twisting

/// Soul Twisting: for a while, binding rituals cost 15 less energy (not below
/// 5) and recharge instantly; it ends after a number of them.
pub struct SoulTwisting;

/// The energy Soul Twisting takes off, and the floor it stops at (the
/// 2026-08-26 update).
const SOUL_TWISTING_DISCOUNT: f64 = 15.0;
const SOUL_TWISTING_FLOOR: f64 = 5.0;

impl HandlerDescribe for SoulTwisting {
    fn describe(&self, _params: &BTreeMap<String, Value>) -> String {
        concat!(
            "For 5…45 seconds, your binding rituals cost 15 less energy (minimum 5) and ",
            "recharge instantly. This ends after 1…3 binding rituals."
        )
        .to_owned()
    }
}

impl SkillHandler for SoulTwisting {
    fn name(&self) -> &'static str {
        "soul_twisting"
    }

    fn on_use(&self, sim: &mut Sim, ctx: &mut ExecCtx) {
        let Some(skill) = ctx.skill else { return };
        let seconds = param(sim, ctx, skill, "duration", Value::Scaled(5, 45));
        let rituals = param(sim, ctx, skill, "rituals", Value::Scaled(1, 3));
        apply_self(
            sim,
            ctx,
            skill,
            EffectKind::Skill,
            seconds,
            Some(rituals.round().clamp(1.0, 255.0) as u8),
        );
    }

    fn on_event(&self, sim: &mut Sim, fired: &Fired, bearer: UnitId, effect: u32) {
        if fired.event != Event::OnSkillUsed || fired.subject != bearer {
            return;
        }
        let Some(used) = fired.skill else { return };
        if !sim.fight.skills[usize::from(used)]
            .skill
            .kind
            .is_a(SkillType::BindingRitual)
        {
            return;
        }
        let Some(active) = sim.units[bearer.index()]
            .effects
            .iter_mut()
            .find(|e| e.id == effect)
        else {
            return;
        };
        let left = match active.charges.first_mut() {
            Some(Some(left)) => {
                *left = left.saturating_sub(1);
                *left
            }
            _ => 0,
        };
        if left == 0 {
            sim.end_effect(bearer, effect, EndReason::Replaced);
        }
    }

    fn adjust_cost(
        &self,
        sim: &Sim,
        _bearer: UnitId,
        _effect: &ActiveEffect,
        skill: u16,
        cost: f64,
    ) -> f64 {
        let ritual = sim.fight.skills[usize::from(skill)]
            .skill
            .kind
            .is_a(SkillType::BindingRitual);
        if ritual && cost > SOUL_TWISTING_FLOOR {
            (cost - SOUL_TWISTING_DISCOUNT).max(SOUL_TWISTING_FLOOR)
        } else {
            cost
        }
    }

    fn adjust_recharge(
        &self,
        sim: &Sim,
        _bearer: UnitId,
        _effect: &ActiveEffect,
        skill: u16,
        recharge_ms: u32,
    ) -> u32 {
        if sim.fight.skills[usize::from(skill)]
            .skill
            .kind
            .is_a(SkillType::BindingRitual)
        {
            0
        } else {
            recharge_ms
        }
    }
}

// ----------------------------------------------------------- Master of Magic

/// Master of Magic: for a while, every elemental attribute is set to a
/// value, and elemental spells return 1 energy plus 30% of their cost.
pub struct MasterOfMagic;

/// The energy an elemental spell returns under Master of Magic.
const MASTER_OF_MAGIC_FLAT: f64 = 1.0;
const MASTER_OF_MAGIC_SHARE: f64 = 0.30;

const ELEMENTAL: [Attribute; 4] = [
    Attribute::AirMagic,
    Attribute::EarthMagic,
    Attribute::FireMagic,
    Attribute::WaterMagic,
];

impl HandlerDescribe for MasterOfMagic {
    fn describe(&self, _params: &BTreeMap<String, Value>) -> String {
        concat!(
            "For 1…61 seconds, all of your elemental attributes are set to 8…14, and your ",
            "elemental spells return 1 energy plus 30% of their energy cost."
        )
        .to_owned()
    }
}

impl SkillHandler for MasterOfMagic {
    fn name(&self) -> &'static str {
        "master_of_magic"
    }

    fn on_use(&self, sim: &mut Sim, ctx: &mut ExecCtx) {
        let Some(skill) = ctx.skill else { return };
        let seconds = param(sim, ctx, skill, "duration", Value::Scaled(1, 61));
        apply_self(sim, ctx, skill, EffectKind::Enchantment, seconds, None);
    }

    fn on_event(&self, sim: &mut Sim, fired: &Fired, bearer: UnitId, _effect: u32) {
        if fired.event != Event::OnSkillUsed || fired.subject != bearer {
            return;
        }
        let Some(used) = fired.skill else { return };
        let spell = &sim.fight.skills[usize::from(used)];
        let elemental = spell.is_spell
            && spell
                .skill
                .attribute
                .is_some_and(|a| ELEMENTAL.contains(&a));
        if elemental {
            let cost = f64::from(spell.skill.cost.energy);
            let back = MASTER_OF_MAGIC_FLAT + MASTER_OF_MAGIC_SHARE * cost;
            sim.gain_energy(bearer, (back * f64::from(ENERGY_SCALE)).round() as i32);
        }
    }

    fn set_rank(&self, sim: &Sim, effect: &ActiveEffect, attribute: Attribute) -> Option<u8> {
        if !ELEMENTAL.contains(&attribute) {
            return None;
        }
        let EffectSource::Handler { skill } = effect.source else {
            return None;
        };
        let params = sim.handler_params(skill);
        let (at0, at15) = match params.get("rank") {
            Some(Value::Scaled(at0, at15)) => (*at0, *at15),
            _ => (8, 14),
        };
        Some(gwsim_data::derived::scaled(at0, at15, effect.rank).clamp(0, 20) as u8)
    }
}

// -------------------------------------------------------- Resurrection Chant

/// Resurrection Chant: raises a party member with up to the caster's current
/// health and a share of its energy.
pub struct ResurrectionChant;

impl HandlerDescribe for ResurrectionChant {
    fn describe(&self, _params: &BTreeMap<String, Value>) -> String {
        concat!(
            "Resurrect target party member with up to your current health and 5…35% energy. ",
            "This spell has half the normal range."
        )
        .to_owned()
    }
}

impl SkillHandler for ResurrectionChant {
    fn name(&self) -> &'static str {
        "resurrection_chant"
    }

    fn on_use(&self, sim: &mut Sim, ctx: &mut ExecCtx) {
        let (Some(skill), Some(target)) = (ctx.skill, ctx.target_unit()) else {
            return;
        };
        if sim.units[target.index()].alive() {
            return;
        }
        let energy_percent = param(sim, ctx, skill, "energy", Value::Scaled(5, 35));
        let health = sim.units[ctx.caster.index()]
            .health_points()
            .min(sim.max_health(target));
        let energy = (f64::from(sim.max_energy(target)) * energy_percent / 100.0).round() as i32;
        sim.resurrect_with(target, health, energy);
    }
}
