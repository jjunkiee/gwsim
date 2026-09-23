//! The effect DSL interpreter (T3.6.7, §8.4).
//!
//! [`Sim::execute`] runs a list of [`Action`]s in an [`ExecCtx`]: who is
//! acting, at whom, with which skill at which rank. Values are evaluated
//! against the caster's ranks (or the rank an effect locked in when it was
//! applied); selectors resolve through the geometry; `Chance` draws from the
//! caster's skill-chance stream.
//!
//! A few actions only make sense inside an effect's `while_active` list —
//! `ModifyStat`, `ReduceIncomingDamage` and the like describe a state, not a
//! deed — and [`Sim::effect_modifiers`] reads them there instead.

use std::collections::BTreeMap;
use std::sync::Arc;

use gwsim_data::core::{ArmorSlot, Attribute, Condition, DamageType, RangeBand, SkillType};
use gwsim_data::dsl::{
    Action, Control, EffectKind, Filter, ModCategory, Quantity, Selector, Side, Stat, Value,
};
use gwsim_data::foe::CreatureTrait;

use crate::damage::{DamageInfo, HealKind};
use crate::effects::{ActiveEffect, ApplyRequest, EffectSource, StackKey};
use crate::log::{LogEvent, LogKind};
use crate::pipeline::InterruptScope;
use crate::rng::Purpose;
use crate::sim::Sim;
use crate::stats::{ModSource, Modifier};
use crate::unit::{Action as UnitAction, ENERGY_SCALE, Target, Team, UnitId, UnitKind};

/// Everything an action needs to know about the moment it runs in.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecCtx {
    pub caster: UnitId,
    pub target: Target,
    /// The skill being used, as a fight skill index.
    pub skill: Option<u16>,
    /// The slot it was used from.
    pub slot: Option<u8>,
    /// The attribute rank `Scaled` values are evaluated at.
    pub rank: u8,
    /// The effect instance, when running a trigger or end action.
    pub effect: Option<u32>,
    /// The other party of the event that started a trigger.
    pub other: Option<UnitId>,
    /// The skill in the event that started a trigger.
    pub event_skill: Option<u16>,
    pub event_amount: f64,
    /// Energy the last `LoseEnergy` or `DrainEnergy` took (Energy Surge).
    pub energy_lost: f64,
    pub health_lost: f64,
    pub hexes_removed: f64,
    pub conditions_removed: f64,
    pub enchantments_removed: f64,
    /// Set by `SetUnblockable` for the attack this skill makes.
    pub unblockable: bool,
    /// Set by `WaiveSacrifice`: the skill's health sacrifice is not paid.
    pub waive_sacrifice: bool,
    /// A per-attack projectile speed factor (Mighty Throw).
    pub projectile_speed: f64,
}

impl ExecCtx {
    /// The context for using a skill.
    pub fn for_skill(
        caster: UnitId,
        target: Target,
        skill: u16,
        slot: Option<u8>,
        rank: u8,
    ) -> Self {
        ExecCtx {
            caster,
            target,
            skill: Some(skill),
            slot,
            rank,
            effect: None,
            other: None,
            event_skill: None,
            event_amount: 0.0,
            energy_lost: 0.0,
            health_lost: 0.0,
            hexes_removed: 0.0,
            conditions_removed: 0.0,
            enchantments_removed: 0.0,
            unblockable: false,
            waive_sacrifice: false,
            projectile_speed: 1.0,
        }
    }

    /// The context for an effect's trigger or end action: the effect's
    /// caster acts, at the unit bearing it, at the rank it locked in.
    pub fn for_effect(effect: &ActiveEffect, bearer: UnitId) -> Self {
        let skill = match effect.source {
            EffectSource::Skill { skill, .. } | EffectSource::Handler { skill } => Some(skill),
            EffectSource::Condition(_) => None,
        };
        ExecCtx {
            effect: Some(effect.id),
            slot: effect.slot,
            ..ExecCtx::for_skill(
                effect.caster,
                Target::Unit(bearer),
                skill.unwrap_or(0),
                None,
                effect.rank,
            )
        }
        .with_skill(skill)
    }

    fn with_skill(mut self, skill: Option<u16>) -> Self {
        self.skill = skill;
        self
    }

    /// The target, if it is a unit.
    pub fn target_unit(&self) -> Option<UnitId> {
        self.target.unit()
    }

    /// The context for an effect definition acting on a bearer, whether from
    /// an effect it bears or a spirit's aura it stands in.
    pub fn for_def(def: &ActiveDef, bearer: UnitId) -> Self {
        ExecCtx {
            effect: def.effect,
            slot: def.slot,
            ..ExecCtx::for_skill(def.caster, Target::Unit(bearer), def.skill, None, def.rank)
        }
    }
}

/// An effect definition acting on a unit: from an effect it bears, or from
/// the aura of a spirit it stands near (T4.3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveDef {
    /// The skill defining it, as a fight skill index.
    pub skill: u16,
    /// Its index among that skill's effect definitions.
    pub def: u16,
    /// Who applied it: the caster of an effect, or the spirit of an aura.
    pub caster: UnitId,
    /// The rank its values use.
    pub rank: u8,
    /// The effect instance, or [`None`] for an aura.
    pub effect: Option<u32>,
    pub slot: Option<u8>,
}

impl Sim {
    /// Runs actions in order.
    pub fn execute(&mut self, actions: &[Action], ctx: &mut ExecCtx) {
        for action in actions {
            self.execute_one(action, ctx);
            if self.outcome.is_some() {
                return;
            }
        }
    }

    fn execute_one(&mut self, action: &Action, ctx: &mut ExecCtx) {
        match action {
            Action::Damage {
                to,
                kind,
                amount,
                armor_ignoring,
            } => {
                let base = self.eval_value(amount, ctx);
                let (targets, factor) = self.select(to, ctx);
                let caster_level = self.units[ctx.caster.index()].level;
                // Typeless and shadow damage ignore armor, and so does holy
                // damage from skills (Damage type); other typed skill damage
                // respects it unless the skill says otherwise.
                let ignores = *armor_ignoring
                    || matches!(kind, None | Some(DamageType::Shadow | DamageType::Holy));
                for target in targets {
                    self.deal_damage(
                        ctx.caster,
                        target,
                        base * factor,
                        DamageInfo {
                            kind: *kind,
                            armor_ignoring: ignores,
                            skill: ctx.skill,
                            attack: false,
                            strike_level: crate::combat::skill_strike_level(caster_level),
                            penetration: 0.0,
                            critical: false,
                            piece: None,
                        },
                    );
                }
            }
            Action::LifeSteal { to, amount } => {
                let value = self.eval_value(amount, ctx);
                let (targets, factor) = self.select(to, ctx);
                for target in targets {
                    let lost = self.health_loss(target, value * factor, Some(ctx.caster));
                    self.heal(
                        ctx.caster,
                        ctx.caster,
                        lost,
                        ctx.skill,
                        HealKind::HealthGain,
                    );
                }
            }
            Action::HealthLoss { to, amount } => {
                let value = self.eval_value(amount, ctx);
                let (targets, factor) = self.select(to, ctx);
                for target in targets {
                    ctx.health_lost = self.health_loss(target, value * factor, Some(ctx.caster));
                }
            }
            Action::Heal { to, amount } => {
                let value = self.eval_value(amount, ctx);
                let (targets, factor) = self.select(to, ctx);
                for target in targets {
                    self.heal(
                        ctx.caster,
                        target,
                        value * factor,
                        ctx.skill,
                        HealKind::Heal,
                    );
                }
            }
            Action::HealthGain { to, amount } => {
                let value = self.eval_value(amount, ctx);
                let (targets, factor) = self.select(to, ctx);
                for target in targets {
                    self.heal(
                        ctx.caster,
                        target,
                        value * factor,
                        ctx.skill,
                        HealKind::HealthGain,
                    );
                }
            }
            Action::SacrificeHealth { percent } => {
                let value = self.eval_value(percent, ctx);
                self.sacrifice(ctx.caster, value);
            }
            Action::GainEnergy { to, amount } => {
                let value = self.eval_value(amount, ctx);
                let (targets, factor) = self.select(to, ctx);
                for target in targets {
                    let units = (value * factor * f64::from(ENERGY_SCALE)).round() as i32;
                    self.gain_energy(target, units);
                }
            }
            Action::LoseEnergy { to, amount } => {
                let (targets, factor) = self.select(to, ctx);
                for target in targets {
                    // Evaluated per creature, so "all of its energy" means its own.
                    let mut each = ctx.clone();
                    each.target = Target::Unit(target);
                    let value = self.eval_value(amount, &each);
                    ctx.energy_lost =
                        f64::from(self.lose_energy(target, (value * factor).round() as i32));
                }
            }
            Action::DrainEnergy { from, amount } => {
                let value = self.eval_value(amount, ctx);
                let (targets, factor) = self.select(from, ctx);
                for target in targets {
                    let lost = self.lose_energy(target, (value * factor).round() as i32);
                    ctx.energy_lost = f64::from(lost);
                    self.gain_energy(ctx.caster, lost * ENERGY_SCALE);
                }
            }
            Action::GainAdrenaline { to, strikes } => {
                let value = self.eval_value(strikes, ctx);
                let (targets, _) = self.select(to, ctx);
                for target in targets {
                    self.gain_adrenaline(target, (value * 25.0).round() as i32);
                }
            }
            Action::LoseAdrenaline { from, strikes } => {
                let value = self.eval_value(strikes, ctx);
                let (targets, _) = self.select(from, ctx);
                for target in targets {
                    self.gain_adrenaline(target, -(value * 25.0).round() as i32);
                }
            }
            Action::ApplyCondition {
                to,
                condition,
                duration,
            } => {
                let ms = (self.eval_value(duration, ctx) * 1000.0).round() as u32;
                let (targets, _) = self.select(to, ctx);
                for target in targets {
                    self.apply_condition(ctx.caster, target, *condition, ms);
                }
            }
            Action::RemoveConditions { from, count, which } => {
                let count = self.eval_value(count, ctx).max(0.0) as usize;
                let (targets, _) = self.select(from, ctx);
                for target in targets {
                    ctx.conditions_removed += self.remove_conditions(target, count, *which) as f64;
                }
            }
            Action::ApplyEffect {
                to,
                effect,
                duration,
            } => self.apply_named_effect(to, effect, duration, ctx),
            Action::RemoveEffects { from, kind, count } => {
                let count = self.eval_value(count, ctx).max(0.0) as usize;
                let (targets, _) = self.select(from, ctx);
                for target in targets {
                    let removed = self.remove_effects(target, *kind, count) as f64;
                    match kind {
                        EffectKind::Hex => ctx.hexes_removed += removed,
                        EffectKind::Enchantment => ctx.enchantments_removed += removed,
                        _ => {}
                    }
                }
            }
            Action::Interrupt { to, disable } => {
                let extra = disable
                    .as_ref()
                    .map(|v| (self.eval_value(v, ctx) * 1000.0).round() as u32);
                let (targets, _) = self.select(to, ctx);
                for target in targets {
                    let slot = self.units[target.index()]
                        .activating()
                        .map(|(slot, _)| slot);
                    if self.interrupt(target, ctx.caster, InterruptScope::Any) {
                        if let (Some(extra), Some(slot)) = (extra, slot)
                            && let Some(state) =
                                self.units[target.index()].bar[usize::from(slot)].as_mut()
                        {
                            state.disabled_until = state.ready_at.plus(extra);
                        }
                        if let Some(skill) = ctx.skill {
                            self.stats.skills[usize::from(skill)].interrupts += 1;
                            if let Some(slot) = self.units[ctx.caster.index()].slot_index {
                                self.stats.slots[slot].interrupts += 1;
                            }
                        }
                    }
                }
            }
            Action::FailSkill { to } => {
                let (targets, _) = self.select(to, ctx);
                for target in targets {
                    if let UnitAction::Activating { failed, .. } =
                        &mut self.units[target.index()].action
                    {
                        *failed = true;
                    }
                }
            }
            Action::KnockDown { to, duration } => {
                let ms = (self.eval_value(duration, ctx) * 1000.0).round() as u32;
                let (targets, _) = self.select(to, ctx);
                for target in targets {
                    self.knock_down(target, ms);
                }
            }
            Action::DisableSkills {
                to,
                which,
                duration,
            } => {
                let ms = (self.eval_value(duration, ctx) * 1000.0).round() as u32;
                let (targets, _) = self.select(to, ctx);
                for target in targets {
                    self.disable_skills(target, *which, ms);
                }
            }
            Action::RechargeSkill { which } => {
                let caster = ctx.caster;
                let slot = match which {
                    None => ctx.slot,
                    Some(slug) => {
                        let fight = Arc::clone(&self.fight);
                        fight
                            .skills
                            .iter()
                            .position(|s| s.slug.as_str() == slug)
                            .and_then(|index| self.units[caster.index()].slot_of(index as u16))
                    }
                };
                if let Some(slot) = slot
                    && let Some(state) = self.units[caster.index()].bar[usize::from(slot)].as_mut()
                {
                    state.ready_at = self.now;
                }
            }
            Action::Resurrect {
                to,
                health_percent,
                energy_percent,
            } => {
                let health = self.eval_value(health_percent, ctx);
                let energy = self.eval_value(energy_percent, ctx);
                let targets = self.select_dead(to, ctx);
                for target in targets {
                    self.resurrect(target, health, energy);
                }
            }
            Action::ShadowStep { to } | Action::Teleport { to } => {
                let (targets, _) = self.select(to, ctx);
                if let Some(target) = targets.first() {
                    let position = self.units[target.index()].pos;
                    let caster = &mut self.units[ctx.caster.index()];
                    caster.pos = position;
                    caster.goal = None;
                }
            }
            Action::SetUnblockable => ctx.unblockable = true,
            Action::Summon { creature, level } => {
                let level = self.eval_value(level, ctx);
                self.summon(ctx.caster, creature.as_str(), level.round() as u8, ctx);
            }
            Action::CreateSpirit {
                spirit,
                level,
                duration,
                attack_damage,
            } => {
                let level = self.eval_value(level, ctx);
                let seconds = self.eval_value(duration, ctx);
                let damage = attack_damage.as_ref().map(|v| self.eval_value(v, ctx));
                self.create_spirit(
                    ctx.caster,
                    spirit.as_str(),
                    level.round() as u8,
                    seconds,
                    damage,
                    ctx,
                );
            }
            Action::HoldBundle { bundle, duration } => {
                let seconds = self.eval_value(duration, ctx);
                self.hold_bundle(ctx.caster, bundle.as_str(), seconds, ctx);
            }
            Action::DropBundle => self.drop_bundle(ctx.caster),
            Action::RunHandler { name } => {
                let fight = Arc::clone(&self.fight);
                if let Some(index) = fight.handlers.index_of(name) {
                    fight.handlers.get(index).on_use(self, ctx);
                }
            }
            Action::EndEffect => {
                if let (Some(id), Some(bearer)) = (ctx.effect, ctx.target_unit()) {
                    self.end_effect(bearer, id, crate::effects::EndReason::Removed);
                }
            }
            Action::WaiveSacrifice => ctx.waive_sacrifice = true,
            Action::Control(control) => self.execute_control(control, ctx),
            // State, not deeds: read by effect_modifiers and the damage
            // pipeline from an effect's while_active list. At the top level
            // of a skill they have nothing to attach to.
            Action::ModifyStat { .. }
            | Action::SetStat { .. }
            | Action::ReduceIncomingDamage { .. }
            | Action::SetCriticalImmune { .. }
            | Action::ModifyRecharge { .. }
            | Action::CreateArea { .. } => {}
        }
    }

    fn execute_control(&mut self, control: &Control, ctx: &mut ExecCtx) {
        match control {
            Control::If {
                condition,
                of,
                then,
                otherwise,
            } => {
                let holds = match of {
                    Some(selector) => {
                        let (units, _) = self.select(selector, ctx);
                        units
                            .iter()
                            .any(|unit| self.filter_passes(condition, *unit, ctx.caster, Some(ctx)))
                    }
                    None => {
                        let judged = ctx.target_unit().unwrap_or(ctx.caster);
                        self.filter_passes(condition, judged, ctx.caster, Some(ctx))
                    }
                };
                if holds {
                    self.execute(then, ctx);
                } else {
                    self.execute(otherwise, ctx);
                }
            }
            Control::ForEach { selector, actions } => {
                let (units, _) = self.select(selector, ctx);
                for unit in units {
                    let mut inner = ctx.clone();
                    inner.target = Target::Unit(unit);
                    self.execute(actions, &mut inner);
                }
            }
            Control::Chance { percent, actions } => {
                let hit = self
                    .streams
                    .unit(ctx.caster, Purpose::SkillChance)
                    .chance(f64::from(*percent) / 100.0);
                if hit {
                    self.execute(actions, ctx);
                }
            }
            Control::Sequence(actions) => self.execute(actions, ctx),
            // Triggers belong to effect definitions, where fire() finds them.
            Control::Triggered { .. } => {}
        }
    }

    pub(crate) fn apply_named_effect(
        &mut self,
        to: &Selector,
        effect: &str,
        duration: &Value,
        ctx: &mut ExecCtx,
    ) {
        let Some(skill) = ctx.skill else { return };
        let fight = Arc::clone(&self.fight);
        let Some(encoding) = &fight.skills[usize::from(skill)].skill.encoding else {
            return;
        };
        let Some(def_index) = encoding.effect_defs.iter().position(|d| d.id == effect) else {
            self.log_event(
                LogEvent::new(self.now, LogKind::Warning)
                    .source(ctx.caster)
                    .detail(&format!("no effect named {effect:?}")),
            );
            return;
        };
        let def = &encoding.effect_defs[def_index];
        let maintained = def.upkeep.is_some();
        let seconds = self.eval_value(duration, ctx);
        let duration_ms = if maintained {
            None
        } else {
            Some((seconds * 1000.0).round().max(0.0) as u32)
        };
        let charges: Vec<Option<u8>> = def
            .triggers
            .iter()
            .map(|trigger| match trigger {
                Control::Triggered { charges, .. } => *charges,
                _ => None,
            })
            .collect();
        let key = match &def.stacking.key {
            Some(name) => StackKey::Named(name.clone()),
            None => StackKey::Def {
                skill,
                def: def_index as u16,
            },
        };
        let (targets, _) = self.select(to, ctx);
        for target in targets {
            self.apply_effect(ApplyRequest {
                target,
                source: EffectSource::Skill {
                    skill,
                    def: def_index as u16,
                },
                kind: def.kind,
                caster: ctx.caster,
                rank: ctx.rank,
                duration_ms,
                upkeep: def.upkeep.unwrap_or(0),
                trigger_count: def.triggers.len(),
                trigger_charges: charges.clone(),
                key: key.clone(),
                rule: def.stacking.rule,
                slot: ctx.slot,
            });
        }
    }

    // ---------------------------------------------------------------- values

    /// Evaluates a value in a context.
    pub fn eval_value(&self, value: &Value, ctx: &ExecCtx) -> f64 {
        match value {
            Value::Fixed(n) => f64::from(*n),
            Value::Scaled(at0, at15) => {
                f64::from(gwsim_data::derived::scaled(*at0, *at15, ctx.rank))
            }
            Value::ScaledBy(attribute, at0, at15) => {
                let rank = self.rank_of(ctx.caster, *attribute);
                f64::from(gwsim_data::derived::scaled(*at0, *at15, rank))
            }
            Value::TitleScaled(track, r0, rmax) => {
                let effective = self
                    .fight
                    .core
                    .title_track(*track)
                    .and_then(|record| record.effective_rank(self.fight.title_rank(*track)))
                    .unwrap_or(0);
                f64::from(gwsim_data::derived::scaled(*r0, *rmax, effective))
            }
            Value::Percent(p) => f64::from(*p),
            Value::PercentOf { percent, of } => {
                f64::from(*percent) / 100.0 * self.quantity(*of, ctx)
            }
            Value::ShareOf { percent, of } => {
                self.eval_value(percent, ctx) / 100.0 * self.quantity(*of, ctx)
            }
            Value::PerUnit { value, of } => self.eval_value(value, ctx) * self.quantity(*of, ctx),
            Value::Min(a, b) => self.eval_value(a, ctx).min(self.eval_value(b, ctx)),
            Value::Max(a, b) => self.eval_value(a, ctx).max(self.eval_value(b, ctx)),
            Value::Sum(values) => values.iter().map(|v| self.eval_value(v, ctx)).sum(),
        }
    }

    fn quantity(&self, quantity: Quantity, ctx: &ExecCtx) -> f64 {
        let target = ctx.target_unit().unwrap_or(ctx.caster);
        match quantity {
            Quantity::EnergyLost => ctx.energy_lost,
            Quantity::HealthLost => ctx.health_lost,
            Quantity::EnergyCost => ctx
                .event_skill
                .map(|skill| f64::from(self.fight.skills[usize::from(skill)].skill.cost.energy))
                .unwrap_or(0.0),
            Quantity::MaxHealth => f64::from(self.max_health(target)),
            Quantity::CurrentHealth => f64::from(self.units[target.index()].health_points()),
            // A spirit's own end actions count its lifetime (Life).
            Quantity::SecondsAlive if ctx.effect.is_none() => {
                let born = self.units[ctx.caster.index()].born_at;
                f64::from(self.now.ms().saturating_sub(born.ms())) / 1000.0
            }
            Quantity::SecondsAlive => {
                let born = ctx
                    .effect
                    .and_then(|id| {
                        self.units[target.index()]
                            .effects
                            .iter()
                            .find(|e| e.id == id)
                            .map(|e| e.applied_at)
                    })
                    .unwrap_or(self.now);
                f64::from(self.now.ms().saturating_sub(born.ms())) / 1000.0
            }
            Quantity::CurrentEnergy => f64::from(self.units[target.index()].energy_points()),
            Quantity::HexesRemoved => ctx.hexes_removed,
            Quantity::ConditionsRemoved => ctx.conditions_removed,
            Quantity::EnchantmentsRemoved => ctx.enchantments_removed,
            Quantity::CreaturesControlled => self
                .units
                .iter()
                .filter(|u| u.alive() && u.master == Some(ctx.caster))
                .count() as f64,
            Quantity::SpiritsInEarshot => {
                let caster = &self.units[ctx.caster.index()];
                let earshot = self.fight.core.gwinches(RangeBand::Earshot);
                self.units
                    .iter()
                    .filter(|u| {
                        u.alive() && u.kind == UnitKind::Spirit && u.pos.within(caster.pos, earshot)
                    })
                    .count() as f64
            }
        }
    }

    /// A unit's current rank in an attribute: its build's rank, plus
    /// temporary effects, less one for Weakness (Condition: ranks above 0
    /// only), clamped to 0..=20.
    pub fn rank_of(&self, unit: UnitId, attribute: Attribute) -> u8 {
        // An effect that sets a rank replaces the build's (Master of Magic).
        if let Some(rank) = self.set_rank(unit, attribute) {
            return rank;
        }
        let base = self.units[unit.index()].base_ranks[attribute.index()];
        let mut rank = f64::from(base);
        for modifier in self.modifiers(unit, Stat::AttributeRank(attribute), None) {
            rank += modifier.value;
        }
        if base > 0 && self.has_condition(unit, Condition::Weakness) {
            rank -= 1.0;
        }
        rank.clamp(0.0, 20.0) as u8
    }

    /// The parameters a skill's handler reference carries.
    pub fn handler_params(&self, skill: u16) -> BTreeMap<String, Value> {
        self.fight.skills[usize::from(skill)]
            .skill
            .encoding
            .as_ref()
            .and_then(|e| e.handler.as_ref())
            .map(|h| h.params.clone())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------- selectors

    /// Resolves a selector to living units, in `UnitId` order, and the share
    /// of the effect they take (1 unless `Secondary`).
    pub fn select(&self, selector: &Selector, ctx: &ExecCtx) -> (Vec<UnitId>, f64) {
        let mut factor = 1.0;
        let mut units = self.resolve(selector, ctx, &mut factor);
        units.sort();
        units.dedup();
        units.retain(|u| self.units[u.index()].alive());
        (units, factor)
    }

    /// Like [`Self::select`], but for dead units (resurrection).
    pub fn select_dead(&self, selector: &Selector, ctx: &ExecCtx) -> Vec<UnitId> {
        let mut factor = 1.0;
        let mut units = self.resolve(selector, ctx, &mut factor);
        units.sort();
        units.dedup();
        units.retain(|u| !self.units[u.index()].alive());
        units
    }

    fn resolve(&self, selector: &Selector, ctx: &ExecCtx, factor: &mut f64) -> Vec<UnitId> {
        let caster = &self.units[ctx.caster.index()];
        match selector {
            Selector::SelfUnit => vec![ctx.caster],
            Selector::Target
            | Selector::TargetFoe
            | Selector::TargetAlly
            | Selector::TargetOtherAlly => ctx.target_unit().into_iter().collect(),
            Selector::Corpse => ctx.target_unit().into_iter().collect(),
            Selector::Other => ctx.other.into_iter().collect(),
            Selector::Around { of, band, side } => {
                let radius = self.fight.core.gwinches(*band);
                let team = match side {
                    Side::Foes => caster.team.other(),
                    Side::Allies => caster.team,
                };
                let centres = self.resolve(of, ctx, factor);
                let mut found = Vec::new();
                for centre in centres {
                    let unit = &self.units[centre.index()];
                    for other in &self.units {
                        if other.id != centre
                            && other.team == team
                            && other.pos.within(unit.pos, radius)
                        {
                            found.push(other.id);
                        }
                    }
                }
                found
            }
            Selector::Location => Vec::new(),
            Selector::Adjacent(inner) => self.area(inner, RangeBand::Adjacent, ctx, factor),
            Selector::Nearby(inner) => self.area(inner, RangeBand::Nearby, ctx, factor),
            Selector::InTheArea(inner) => self.area(inner, RangeBand::InTheArea, ctx, factor),
            Selector::Earshot(inner) => self.area(inner, RangeBand::Earshot, ctx, factor),
            Selector::SpiritRange(inner) => self.area(inner, RangeBand::SpiritRange, ctx, factor),
            Selector::InRangeOf(inner) => {
                let centres = self.resolve(inner, ctx, factor);
                let mut found = Vec::new();
                for centre in centres {
                    let unit = &self.units[centre.index()];
                    let radius = self.aura_range(centre);
                    for other in &self.units {
                        if other.alive()
                            && other.team == unit.team
                            && !other.kind.is_summoned()
                            && other.pos.within(unit.pos, radius)
                        {
                            found.push(other.id);
                        }
                    }
                }
                found
            }
            Selector::Party => {
                let range = self.fight.core.gwinches(RangeBand::Party);
                self.units
                    .iter()
                    .filter(|u| {
                        u.team == caster.team
                            && !u.kind.is_summoned()
                            && u.pos.within(caster.pos, range)
                    })
                    .map(|u| u.id)
                    .collect()
            }
            Selector::PartyInRange(band) => {
                let range = self.fight.core.gwinches(*band);
                self.units
                    .iter()
                    .filter(|u| {
                        u.team == caster.team
                            && !u.kind.is_summoned()
                            && u.pos.within(caster.pos, range)
                    })
                    .map(|u| u.id)
                    .collect()
            }
            Selector::Foes => self
                .units
                .iter()
                .filter(|u| u.team != caster.team)
                .map(|u| u.id)
                .collect(),
            Selector::Allies => self
                .units
                .iter()
                .filter(|u| u.team == caster.team)
                .map(|u| u.id)
                .collect(),
            Selector::Spirits => self
                .units
                .iter()
                .filter(|u| u.kind == UnitKind::Spirit)
                .map(|u| u.id)
                .collect(),
            Selector::Minions => self
                .units
                .iter()
                .filter(|u| u.kind == UnitKind::Minion)
                .map(|u| u.id)
                .collect(),
            Selector::Nearest(inner) => {
                let candidates = self.resolve(inner, ctx, factor);
                candidates
                    .into_iter()
                    .filter(|u| *u != ctx.caster && self.units[u.index()].alive())
                    .min_by(|a, b| {
                        let da = self.units[a.index()].pos.distance_squared(caster.pos);
                        let db = self.units[b.index()].pos.distance_squared(caster.pos);
                        da.total_cmp(&db).then(a.cmp(b))
                    })
                    .into_iter()
                    .collect()
            }
            Selector::Filtered { of, filter } => {
                let candidates = self.resolve(of, ctx, factor);
                candidates
                    .into_iter()
                    .filter(|u| self.filter_passes(filter, *u, ctx.caster, Some(ctx)))
                    .collect()
            }
            Selector::Secondary { of, factor: share } => {
                *factor *= f64::from(*share);
                let main = ctx.target_unit();
                self.resolve(of, ctx, factor)
                    .into_iter()
                    .filter(|u| Some(*u) != main)
                    .collect()
            }
        }
    }

    /// Units within a band of each centre. Around a foe of the user, the
    /// area reaches that foe and its fellows; around the user or an ally, it
    /// reaches the user's foes, and not the centre itself (effect-dsl.md).
    fn area(
        &self,
        inner: &Selector,
        band: RangeBand,
        ctx: &ExecCtx,
        factor: &mut f64,
    ) -> Vec<UnitId> {
        let radius = self.fight.core.gwinches(band);
        let user_team = self.units[ctx.caster.index()].team;
        let centres = self.resolve(inner, ctx, factor);
        let mut found = Vec::new();
        for centre in centres {
            let unit = &self.units[centre.index()];
            let hostile_centre = unit.team != user_team;
            let team = if hostile_centre {
                unit.team
            } else {
                user_team.other()
            };
            for other in &self.units {
                if other.team == team
                    && (hostile_centre || other.id != centre)
                    && other.pos.within(unit.pos, radius)
                {
                    found.push(other.id);
                }
            }
        }
        found
    }

    /// The radius of a spirit's aura, from `spirits.ron` (A-015), or spirit
    /// range.
    pub fn aura_range(&self, spirit: UnitId) -> f32 {
        self.units[spirit.index()]
            .creature_type
            .as_ref()
            .and_then(|slug| self.fight.spirits.get(slug))
            .and_then(|spec| spec.range)
            .unwrap_or_else(|| self.fight.core.gwinches(RangeBand::SpiritRange))
    }

    /// Every effect definition acting on a unit: the effects it bears, then
    /// the auras of spirits whose range it stands in (T4.3.7). Spirits are
    /// untouched by auras; a binding ritual reaches its own side, a nature
    /// ritual every non-spirit creature.
    pub fn defs_on(&self, unit: UnitId) -> Vec<ActiveDef> {
        let me = &self.units[unit.index()];
        let mut found: Vec<ActiveDef> = me
            .effects
            .iter()
            .filter_map(|effect| match effect.source {
                EffectSource::Skill { skill, def } => Some(ActiveDef {
                    skill,
                    def,
                    caster: effect.caster,
                    rank: effect.rank,
                    effect: Some(effect.id),
                    slot: effect.slot,
                }),
                _ => None,
            })
            .collect();
        if me.kind == UnitKind::Spirit || me.has_trait(CreatureTrait::Spirit) {
            return found;
        }
        for spirit in &self.units {
            let Some(aura) = spirit.aura else { continue };
            if !spirit.alive()
                || (!aura.affects_all && spirit.team != me.team)
                || !me.pos.within(spirit.pos, self.aura_range(spirit.id))
            {
                continue;
            }
            let Some(encoding) = &self.fight.skills[usize::from(aura.skill)].skill.encoding else {
                continue;
            };
            for (index, def) in encoding.effect_defs.iter().enumerate() {
                if def.kind == EffectKind::SpiritAura {
                    found.push(ActiveDef {
                        skill: aura.skill,
                        def: index as u16,
                        caster: spirit.id,
                        rank: aura.rank,
                        effect: None,
                        slot: None,
                    });
                }
            }
        }
        found
    }

    /// The definition an active def points at.
    pub fn def_of(&self, def: &ActiveDef) -> Option<&gwsim_data::dsl::EffectDef> {
        self.fight.skills[usize::from(def.skill)]
            .skill
            .encoding
            .as_ref()
            .and_then(|e| e.effect_defs.get(usize::from(def.def)))
    }

    // --------------------------------------------------------------- filters

    /// Whether a unit passes a filter, judged relative to `relative_to` (the
    /// caster, for "hostile", "owned" and the like).
    pub fn filter_passes(
        &self,
        filter: &Filter,
        unit: UnitId,
        relative_to: UnitId,
        ctx: Option<&ExecCtx>,
    ) -> bool {
        let u = &self.units[unit.index()];
        let me = &self.units[relative_to.index()];
        match filter {
            Filter::Hexed => self.has_kind(unit, EffectKind::Hex),
            Filter::Enchanted => self.has_kind(unit, EffectKind::Enchantment),
            Filter::HasCondition(condition) => self.has_condition(unit, *condition),
            Filter::Casting => u.activating().is_some(),
            Filter::CastingSpell => self.casting_spell(unit),
            Filter::Attacking => {
                matches!(u.action, UnitAction::Attacking { .. })
                    || u.activating().is_some_and(|(slot, _)| {
                        self.slot_skill(unit, slot)
                            .is_some_and(|s| self.fight.skills[usize::from(s)].is_attack)
                    })
            }
            Filter::Moving => u.moving(),
            Filter::KnockedDown => u.knocked_down(),
            Filter::BelowHealth { percent } => {
                self.health_fraction(unit) * 100.0 < f64::from(*percent)
            }
            Filter::AboveHealth { percent } => {
                self.health_fraction(unit) * 100.0 > f64::from(*percent)
            }
            Filter::CreatureType(name) => {
                u.species.eq_ignore_ascii_case(name)
                    || u.traits
                        .iter()
                        .any(|t| format!("{t:?}").eq_ignore_ascii_case(name))
            }
            Filter::IsSpirit => u.kind == UnitKind::Spirit || u.has_trait(CreatureTrait::Spirit),
            Filter::IsSummoned => u.kind.is_summoned(),
            Filter::IsMinion => u.kind == UnitKind::Minion,
            Filter::HoldingMartialWeapon => u.weapon.as_ref().is_some_and(|w| !w.caster),
            Filter::HoldingCasterWeapon => u.holds_caster_weapon(),
            Filter::Owned => u.master == Some(relative_to),
            Filter::Hostile => u.team != me.team,
            Filter::Allied => u.team == me.team,
            Filter::RechargingSkills { at_least } => {
                let recharging = u
                    .bar
                    .iter()
                    .flatten()
                    .filter(|s| s.ready_at > self.now)
                    .count();
                recharging >= usize::from(*at_least)
            }
            Filter::ControllingMinions { at_least } => {
                self.controlled(unit, UnitKind::Minion) >= usize::from(*at_least)
            }
            Filter::ControllingSpirits { at_least } => {
                self.controlled(unit, UnitKind::Spirit) >= usize::from(*at_least)
            }
            Filter::ExploitsCorpse => ctx
                .and_then(|c| c.skill)
                .is_some_and(|s| self.fight.skills[usize::from(s)].skill.flags.needs_corpse),
            Filter::Removed(kind) => ctx.is_some_and(|c| match kind {
                EffectKind::Hex => c.hexes_removed > 0.0,
                EffectKind::Enchantment => c.enchantments_removed > 0.0,
                EffectKind::Condition => c.conditions_removed > 0.0,
                _ => false,
            }),
            Filter::SpiritsInEarshot { at_least } => {
                let earshot = self.fight.core.gwinches(RangeBand::Earshot);
                self.units
                    .iter()
                    .filter(|o| {
                        o.alive() && o.kind == UnitKind::Spirit && o.pos.within(u.pos, earshot)
                    })
                    .count()
                    >= usize::from(*at_least)
            }
            Filter::CorpsesInEarshot { at_least } => {
                let earshot = self.fight.core.gwinches(RangeBand::Earshot);
                self.units
                    .iter()
                    .filter(|o| !o.alive() && o.corpse_available && o.pos.within(u.pos, earshot))
                    .count()
                    >= usize::from(*at_least)
            }
            Filter::NearAllies => {
                let nearby = self.fight.core.gwinches(RangeBand::Nearby);
                self.units.iter().any(|o| {
                    o.alive() && o.id != unit && o.team == me.team && o.pos.within(u.pos, nearby)
                })
            }
            Filter::Not(inner) => !self.filter_passes(inner, unit, relative_to, ctx),
            Filter::All(filters) => filters
                .iter()
                .all(|f| self.filter_passes(f, unit, relative_to, ctx)),
            Filter::Any(filters) => filters
                .iter()
                .any(|f| self.filter_passes(f, unit, relative_to, ctx)),
        }
    }

    fn controlled(&self, master: UnitId, kind: UnitKind) -> usize {
        self.units
            .iter()
            .filter(|u| u.alive() && u.kind == kind && u.master == Some(master))
            .count()
    }

    /// Whether a unit is activating a spell.
    pub fn casting_spell(&self, unit: UnitId) -> bool {
        self.units[unit.index()]
            .activating()
            .and_then(|(slot, _)| self.slot_skill(unit, slot))
            .is_some_and(|skill| self.fight.skills[usize::from(skill)].is_spell)
    }

    /// The skill in a unit's slot.
    pub fn slot_skill(&self, unit: UnitId, slot: u8) -> Option<u16> {
        self.units[unit.index()].bar[usize::from(slot)].map(|s| s.skill)
    }

    /// Health as a fraction of maximum.
    pub fn health_fraction(&self, unit: UnitId) -> f64 {
        let max = self.max_health(unit).max(1);
        f64::from(self.units[unit.index()].health) / f64::from(max * crate::unit::HEALTH_SCALE)
    }

    // ------------------------------------------------------------- modifiers

    /// Every modifier on a stat for a unit (T3.6.5): permanent ones, gear
    /// whose conditions hold now, the `while_active` modifiers of its
    /// effects, and its conditions. `piece` narrows armor to one piece's
    /// insignia.
    pub fn modifiers(&self, unit: UnitId, stat: Stat, piece: Option<ArmorSlot>) -> Vec<Modifier> {
        let u = &self.units[unit.index()];
        let mut found: Vec<Modifier> = u
            .permanent
            .iter()
            .filter(|m| m.stat == stat)
            .copied()
            .collect();

        for gear in &u.gear {
            if gear.piece.is_some() && gear.piece != piece && stat == Stat::Armor {
                continue;
            }
            let ctx = ExecCtx::for_skill(unit, Target::Unit(unit), 0, None, 0);
            self.collect_modifiers(
                &gear.actions,
                stat,
                &ctx,
                ModSource::Gear,
                false,
                Share::All,
                &mut found,
            );
        }

        for active in self.defs_on(unit) {
            let Some(def) = self.def_of(&active) else {
                continue;
            };
            let ctx = ExecCtx::for_def(&active, unit);
            // A `ModifyStat(to: Self)` in a hex helps its caster, not the
            // bearer (Life Siphon); the caster collects it below.
            let bearer_share = if active.caster == unit {
                Share::All
            } else {
                Share::Bearer
            };
            self.collect_modifiers(
                &def.while_active,
                stat,
                &ctx,
                ModSource::Effect(active.effect.unwrap_or(u32::MAX)),
                true,
                bearer_share,
                &mut found,
            );
        }
        // The caster's share of effects it put on others.
        for other in &self.units {
            if other.id == unit {
                continue;
            }
            for effect in &other.effects {
                let EffectSource::Skill { skill, def } = effect.source else {
                    continue;
                };
                if effect.caster != unit {
                    continue;
                }
                let Some(def) = self.fight.skills[usize::from(skill)]
                    .skill
                    .encoding
                    .as_ref()
                    .and_then(|e| e.effect_defs.get(usize::from(def)))
                else {
                    continue;
                };
                let ctx = ExecCtx::for_effect(effect, other.id);
                self.collect_modifiers(
                    &def.while_active,
                    stat,
                    &ctx,
                    ModSource::Effect(effect.id),
                    true,
                    Share::Caster,
                    &mut found,
                );
            }
        }

        // Conditions with a fixed effect on a stat (Condition).
        let condition = |c: Condition| {
            u.effects
                .iter()
                .any(|e| e.source == EffectSource::Condition(c))
        };
        let fixed = |value: f64, category: ModCategory| Modifier {
            stat,
            value,
            category,
            exceeds_cap: false,
            source: ModSource::Condition,
        };
        match stat {
            Stat::MovementSpeed if condition(Condition::Crippled) => {
                found.push(fixed(-50.0, ModCategory::Multiplicative));
            }
            Stat::Armor if condition(Condition::CrackedArmor) => {
                found.push(fixed(-20.0, ModCategory::Bonus))
            }
            Stat::HealingReceived if condition(Condition::DeepWound) => {
                found.push(fixed(-20.0, ModCategory::Multiplicative));
            }
            _ => {}
        }
        found
    }

    /// Reads `ModifyStat` actions out of a list, following `If`s whose
    /// conditions hold for the bearer. `Chance` nodes are left to the moment
    /// they roll (weapon chance mods, in the pipeline).
    #[allow(clippy::too_many_arguments)]
    fn collect_modifiers(
        &self,
        actions: &[Action],
        stat: Stat,
        ctx: &ExecCtx,
        source: ModSource,
        from_skill: bool,
        share: Share,
        found: &mut Vec<Modifier>,
    ) {
        for action in actions {
            match action {
                Action::ModifyStat {
                    to,
                    stat: s,
                    amount,
                    category,
                } if stat_matches(*s, stat) && share.takes(to) => found.push(Modifier {
                    stat,
                    value: self.eval_value(amount, ctx),
                    category: *category,
                    exceeds_cap: from_skill && *category == ModCategory::Special,
                    source,
                }),
                Action::Control(control) => match control.as_ref() {
                    Control::If {
                        condition,
                        of,
                        then,
                        otherwise,
                    } => {
                        let judged = of
                            .as_ref()
                            .and_then(|selector| {
                                let (units, _) = self.select(selector, ctx);
                                units.first().copied()
                            })
                            .unwrap_or(ctx.target_unit().unwrap_or(ctx.caster));
                        let bearer = ctx.target_unit().unwrap_or(ctx.caster);
                        let branch = if self.filter_passes(condition, judged, bearer, Some(ctx)) {
                            then
                        } else {
                            otherwise
                        };
                        self.collect_modifiers(branch, stat, ctx, source, from_skill, share, found);
                    }
                    Control::Sequence(inner) => {
                        self.collect_modifiers(inner, stat, ctx, source, from_skill, share, found)
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    /// The weapon chance mods on a unit for a stat: the highest single chance
    /// and its amount (A-038: the same kind does not stack).
    pub fn chance_mod(&self, unit: UnitId, stat: Stat) -> Option<(f64, f64)> {
        let mut best: Option<(f64, f64)> = None;
        for gear in &self.units[unit.index()].gear {
            for action in &gear.actions {
                let Action::Control(control) = action else {
                    continue;
                };
                let Control::Chance { percent, actions } = control.as_ref() else {
                    continue;
                };
                for inner in actions {
                    if let Action::ModifyStat {
                        stat: s, amount, ..
                    } = inner
                        && *s == stat
                    {
                        let ctx = ExecCtx::for_skill(unit, Target::Unit(unit), 0, None, 0);
                        let value = self.eval_value(amount, &ctx);
                        let chance = f64::from(*percent) / 100.0;
                        if best.is_none_or(|(c, _)| chance > c) {
                            best = Some((chance, value));
                        }
                    }
                }
            }
        }
        best
    }

    /// Damage reductions on a target from its effects' `while_active` lists,
    /// in effect order: `(flat, percent, cap share of max health, only
    /// from, caster, skill)`.
    pub fn damage_reductions(&self, unit: UnitId) -> Vec<crate::damage::Reduction> {
        let mut found = Vec::new();
        for active in self.defs_on(unit) {
            let Some(def) = self.def_of(&active) else {
                continue;
            };
            let ctx = ExecCtx::for_def(&active, unit);
            for action in &def.while_active {
                if let Action::ReduceIncomingDamage {
                    flat,
                    percent,
                    cap_percent_of_max_health,
                    only_from,
                    limit,
                    heals,
                    cost_to_source,
                    ..
                } = action
                {
                    found.push(crate::damage::Reduction {
                        flat: flat.as_ref().map(|v| self.eval_value(v, &ctx)),
                        percent: percent.as_ref().map(|v| self.eval_value(v, &ctx)),
                        cap: cap_percent_of_max_health
                            .as_ref()
                            .map(|v| self.eval_value(v, &ctx)),
                        only_from: *only_from,
                        limit: limit.as_ref().map(|v| self.eval_value(v, &ctx)),
                        heals: *heals,
                        cost_to_source: cost_to_source.as_ref().map(|v| self.eval_value(v, &ctx)),
                        caster: active.caster,
                        skill: active.skill,
                    });
                }
            }
        }
        found
    }

    /// Whether a unit is immune to critical hits.
    pub fn critical_immune(&self, unit: UnitId) -> bool {
        self.defs_on(unit).iter().any(|active| {
            self.def_of(active).is_some_and(|d| {
                d.while_active
                    .iter()
                    .any(|a| matches!(a, Action::SetCriticalImmune { .. }))
            })
        })
    }

    /// Whether a skill type counts as a spell for Fast Casting and Dazed.
    pub fn is_spell_type(kind: SkillType) -> bool {
        kind.is_a(SkillType::Spell)
    }

    /// The team of a unit.
    pub fn team(&self, unit: UnitId) -> Team {
        self.units[unit.index()].team
    }

    /// Damage types for caster weapons (A-037).
    pub fn caster_damage_type(profession: gwsim_data::core::Profession) -> DamageType {
        use gwsim_data::core::Profession as P;
        match profession {
            P::Mesmer => DamageType::Chaos,
            P::Necromancer => DamageType::Dark,
            P::Monk => DamageType::Holy,
            P::Ritualist => DamageType::Lightning,
            P::Elementalist => DamageType::Fire,
            _ => DamageType::Chaos,
        }
    }
}

/// Whether a modifier's stat reaches the stat asked for. `ElementalAttributes`
/// reaches each elemental attribute (Master of Magic).
fn stat_matches(have: Stat, want: Stat) -> bool {
    have == want
        || matches!(
            (have, want),
            (
                Stat::ElementalAttributes,
                Stat::AttributeRank(
                    Attribute::AirMagic
                        | Attribute::EarthMagic
                        | Attribute::FireMagic
                        | Attribute::WaterMagic
                )
            )
        )
}

/// Which `ModifyStat` actions of an effect a unit takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Share {
    /// All of them: gear, and effects the unit cast on itself.
    All,
    /// The bearer's: everything but `to: Self`, which means the caster.
    Bearer,
    /// The caster's: only `to: Self`.
    Caster,
}

impl Share {
    fn takes(self, to: &Selector) -> bool {
        let caster_side = matches!(to, Selector::SelfUnit);
        match self {
            Share::All => true,
            Share::Bearer => !caster_side,
            Share::Caster => caster_side,
        }
    }
}
