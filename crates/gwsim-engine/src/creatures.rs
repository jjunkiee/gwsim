//! Created creatures and held items: spirits, minions, bundles, and
//! resurrection (T4.3.7, T4.3.8, T4.3.10).
//!
//! **Spirits** stand where they were made, carry their ritual's aura to
//! everything in range (the aura's definitions are read by
//! [`Sim::defs_on`]), and die when their time is up. Only one allied spirit
//! of a type may stand: a new one destroys the old, and a nature ritual also
//! destroys an enemy spirit of the same type (ENG-32).
//!
//! **Minions** rise from exploitable corpses, follow and fight for their
//! master, decay one pip faster every 20 seconds, and are capped at
//! 2 + Death Magic / 2 per master, the oldest crumbling first.
//!
//! **Bundles** are an effect of kind `Bundle` on the holder: holding one is
//! the effect lasting, dropping it is the effect ending, and "when you drop
//! it" is that effect's `on_end`.

use std::sync::Arc;

use gwsim_data::core::{Attribute, TitleTrack};
use gwsim_data::dsl::{EffectKind, Event, Selector, Value};
use gwsim_data::foe::{AuraReach, CreatureTrait};

use crate::ai::Controller;
use crate::effects::{EffectSource, EndReason};
use crate::exec::ExecCtx;
use crate::log::{LogEvent, LogKind};
use crate::sim::{Fired, Sim};
use crate::time::EventKind;
use gwsim_data::derived::PerDamageType;

use crate::unit::{
    Action, Aura, ENERGY_SCALE, HEALTH_SCALE, Team, UnitId, UnitKind, WeaponProfile,
};

/// Spirit energy and pips (Spirit): 31 energy, 41 for hostile spirits in hard
/// mode, and four pips.
const SPIRIT_ENERGY: i32 = 31;
const HOSTILE_HM_SPIRIT_ENERGY: i32 = 41;
const SPIRIT_ENERGY_PIPS: i32 = 4;

impl Sim {
    /// Raises a dead unit with a share of its health and energy (§10.11).
    pub fn resurrect(&mut self, unit: UnitId, health_percent: f64, energy_percent: f64) {
        if self.units[unit.index()].alive() {
            return;
        }
        let max_health = self.max_health(unit);
        let max_energy = self.max_energy(unit);
        self.resurrect_with(
            unit,
            (f64::from(max_health) * health_percent / 100.0).round() as i32,
            (f64::from(max_energy) * energy_percent / 100.0).round() as i32,
        );
    }

    /// Raises a dead unit with set health and energy, in whole points.
    pub fn resurrect_with(&mut self, unit: UnitId, health: i32, energy: i32) {
        if self.units[unit.index()].alive() {
            return;
        }
        let max_health = self.max_health(unit);
        let now = self.now;
        let u = &mut self.units[unit.index()];
        u.action = Action::Idle;
        u.action_generation = u.action_generation.wrapping_add(1);
        u.health = health.clamp(1, max_health.max(1)) * HEALTH_SCALE;
        u.energy = energy.max(0) * ENERGY_SCALE;
        u.dead_at = None;
        u.corpse_available = false;
        u.next_decision_at = now;
        self.log_event(
            LogEvent::new(now, LogKind::Heal)
                .target(unit)
                .detail("resurrected"),
        );
    }

    // ------------------------------------------------------------ spirits

    /// Creates a spirit where its creator stands (T4.3.7).
    pub fn create_spirit(
        &mut self,
        caster: UnitId,
        slug: &str,
        level: u8,
        seconds: f64,
        attack_damage: Option<f64>,
        ctx: &ExecCtx,
    ) {
        let fight = Arc::clone(&self.fight);
        let spec = fight.spirits.get(slug);
        let affects_all = spec.is_some_and(|s| s.affects == AuraReach::All);
        let team = self.units[caster.index()].team;

        // One allied spirit of each type; nature rituals also replace an
        // enemy spirit of the same type (ENG-32).
        let replaced: Vec<UnitId> = self
            .units
            .iter()
            .filter(|u| {
                u.alive()
                    && u.kind == UnitKind::Spirit
                    && u.creature_type.as_deref() == Some(slug)
                    && (u.team == team || affects_all)
            })
            .map(|u| u.id)
            .collect();
        for old in replaced {
            self.kill(old, None);
        }

        self.touch(15);
        let hostile_hm = fight.hard_mode && team == Team::Foes;
        let spawning = self.rank_of(caster, Attribute::SpawningPower);
        let health = (f64::from(20 * u32::from(level)) * (1.0 + 0.04 * f64::from(spawning)))
            .round()
            .max(1.0) as i32;
        let armor: i16 = if hostile_hm {
            100
        } else {
            2 + 6 * i16::from(level)
        };
        let energy = if hostile_hm {
            HOSTILE_HM_SPIRIT_ENERGY
        } else {
            SPIRIT_ENERGY
        };
        let name = spec
            .map(|s| s.name.clone())
            .unwrap_or_else(|| slug.to_owned());

        let id = UnitId(self.units.len() as u16);
        let mut unit = crate::setup::blank_unit(
            id,
            name,
            team,
            UnitKind::Spirit,
            level,
            fight.tunables.collision_radius,
        );
        unit.pos = self.units[caster.index()].pos;
        unit.stationary = true;
        unit.master = Some(caster);
        unit.species = "Spirit".to_owned();
        unit.traits = vec![CreatureTrait::Spirit];
        unit.natural_regeneration = false;
        unit.base_max_health = health;
        unit.health = health * HEALTH_SCALE;
        unit.base_max_energy = energy;
        unit.energy = energy * ENERGY_SCALE;
        unit.base_energy_pips = SPIRIT_ENERGY_PIPS;
        unit.armor = [PerDamageType {
            base: armor,
            by_type: [armor; 11],
        }; 5];
        unit.born_at = self.now;
        unit.next_decision_at = self.now;
        unit.creature_type = Some(slug.to_owned());
        let has_aura = ctx.skill.is_some_and(|skill| {
            fight.skills[usize::from(skill)]
                .skill
                .encoding
                .as_ref()
                .is_some_and(|e| {
                    e.effect_defs
                        .iter()
                        .any(|d| d.kind == EffectKind::SpiritAura)
                })
        });
        if has_aura && let Some(skill) = ctx.skill {
            unit.aura = Some(Aura {
                skill,
                rank: ctx.rank,
                affects_all,
            });
        }
        if let (Some(damage), Some(attack)) = (attack_damage, spec.and_then(|s| s.attack.as_ref()))
        {
            // Spirit attacks deal armor-ignoring damage (Spirit).
            let damage = damage.round() as i32;
            unit.weapon = Some(WeaponProfile {
                slug: "spirit-attack".to_owned(),
                damage: (damage, damage),
                damage_type: gwsim_data::core::DamageType::Chaos,
                interval_ms: attack.interval.ms(),
                range: attack.range,
                projectile_speed: fight.tunables.projectile_speeds.get("spell").copied(),
                mastery: None,
                caster: false,
                armor_ignoring: true,
            });
        }
        self.add_creature(unit, Controller::Spirit, caster);
        self.queue.schedule(
            self.now.plus((seconds * 1000.0).round().max(0.0) as u32),
            EventKind::CreatureExpiry { unit: id },
        );
    }

    // ------------------------------------------------------------ minions

    /// Animates a minion from the nearest exploitable corpse (T4.3.8).
    pub fn summon(&mut self, caster: UnitId, slug: &str, level: u8, ctx: &ExecCtx) {
        let fight = Arc::clone(&self.fight);
        let Some(spec) = fight.minions.get(slug) else {
            self.log_event(
                LogEvent::new(self.now, LogKind::Warning)
                    .source(caster)
                    .detail(&format!("no minion {slug:?} in minions.ron")),
            );
            return;
        };
        let needs_corpse = ctx
            .skill
            .is_some_and(|s| fight.skills[usize::from(s)].skill.flags.needs_corpse);
        let position = if needs_corpse {
            let Some(corpse) = self.nearest_corpse(caster) else {
                return;
            };
            self.units[corpse.index()].corpse_available = false;
            self.units[corpse.index()].pos
        } else {
            self.units[caster.index()].pos
        };

        // The control cap, checked only when a minion is made; the oldest
        // goes first (Minion).
        let cap = 2 + usize::from(self.rank_of(caster, Attribute::DeathMagic) / 2);
        let mut owned: Vec<(crate::time::SimTime, UnitId)> = self
            .units
            .iter()
            .filter(|u| u.alive() && u.kind == UnitKind::Minion && u.master == Some(caster))
            .map(|u| (u.born_at, u.id))
            .collect();
        owned.sort();
        while owned.len() >= cap {
            let (_, oldest) = owned.remove(0);
            self.kill(oldest, None);
        }

        let spawning = self.rank_of(caster, Attribute::SpawningPower);
        let health = (f64::from(fight.core.levels.health_at(level))
            * (1.0 + 0.04 * f64::from(spawning)))
        .round()
        .max(1.0) as i32;
        let armor: i16 = match (spec.armor_per_level, spec.armor) {
            (Some((slope, intercept)), _) => (slope * f32::from(level) + intercept).round() as i16,
            (None, Some(armor)) => armor,
            // The Minion page's general formula.
            (None, None) => (3.75 * f32::from(level) + 5.0).round() as i16,
        };
        let team = self.units[caster.index()].team;
        let id = UnitId(self.units.len() as u16);
        let mut unit = crate::setup::blank_unit(
            id,
            spec.name.clone(),
            team,
            UnitKind::Minion,
            level,
            fight.tunables.collision_radius,
        );
        unit.pos = position;
        unit.master = Some(caster);
        unit.species = "Undead".to_owned();
        unit.traits = spec.traits.clone();
        unit.natural_regeneration = false;
        unit.base_max_health = health;
        unit.health = health * HEALTH_SCALE;
        unit.armor = [PerDamageType {
            base: armor,
            by_type: [armor; 11],
        }; 5];
        unit.weapon = fight.minion_weapons.get(slug).cloned();
        unit.born_at = self.now;
        unit.next_decision_at = self.now;
        unit.creature_type = Some(slug.to_owned());
        self.touch(43);
        self.add_creature(unit, Controller::Minion, caster);
    }

    /// The nearest exploitable corpse within casting range of a unit.
    pub fn nearest_corpse(&self, unit: UnitId) -> Option<UnitId> {
        let me = self.units[unit.index()].pos;
        let range = self
            .fight
            .core
            .gwinches(gwsim_data::core::RangeBand::Casting);
        self.units
            .iter()
            .filter(|u| !u.alive() && u.corpse_available && u.pos.within(me, range))
            .min_by(|a, b| {
                a.pos
                    .distance_squared(me)
                    .total_cmp(&b.pos.distance_squared(me))
                    .then(a.id.cmp(&b.id))
            })
            .map(|u| u.id)
    }

    /// Adds a created creature to the fight.
    fn add_creature(&mut self, unit: crate::unit::Unit, controller: Controller, creator: UnitId) {
        let id = unit.id;
        if self.logging() {
            self.log_event(
                LogEvent::new(self.now, LogKind::Decision)
                    .source(creator)
                    .target(id)
                    .detail(&format!("created {} (level {})", unit.name, unit.level)),
            );
        }
        self.units.push(unit);
        self.controllers.push(controller);
        self.fire(Fired {
            event: Event::OnCreatureCreated,
            subject: creator,
            other: Some(id),
            skill: None,
            amount: 0.0,
        });
    }

    /// What a creature's death sets off: a spirit's aura end actions (Life),
    /// and the spirit-death event.
    pub(crate) fn creature_died(&mut self, unit: UnitId) {
        let u = &self.units[unit.index()];
        if u.kind != UnitKind::Spirit {
            return;
        }
        let master = u.master;
        if let Some(aura) = u.aura {
            let fight = Arc::clone(&self.fight);
            if let Some(encoding) = &fight.skills[usize::from(aura.skill)].skill.encoding {
                for def in encoding
                    .effect_defs
                    .iter()
                    .filter(|d| d.kind == EffectKind::SpiritAura && !d.on_end.is_empty())
                {
                    let mut ctx = ExecCtx::for_skill(
                        unit,
                        crate::unit::Target::Unit(unit),
                        aura.skill,
                        None,
                        aura.rank,
                    );
                    self.execute(&def.on_end, &mut ctx);
                }
            }
        }
        if let Some(master) = master {
            self.fire(Fired {
                event: Event::OnSpiritDeath,
                subject: master,
                other: Some(unit),
                skill: None,
                amount: 0.0,
            });
        }
    }

    // ------------------------------------------------------------ bundles

    /// Picks up a bundle: applies the skill's `Bundle` effect of that name to
    /// the caster, replacing any bundle it holds (one at a time).
    pub fn hold_bundle(&mut self, _holder: UnitId, bundle: &str, seconds: f64, ctx: &mut ExecCtx) {
        let duration = Value::Fixed(seconds.round() as i32);
        self.apply_named_effect(&Selector::SelfUnit, bundle, &duration, ctx);
    }

    /// Drops the held bundle, which ends its effect and runs its `on_end`.
    pub fn drop_bundle(&mut self, caster: UnitId) {
        let held: Vec<u32> = self.units[caster.index()]
            .effects
            .iter()
            .filter(|e| e.kind == EffectKind::Bundle)
            .map(|e| e.id)
            .collect();
        for id in held {
            self.end_effect(caster, id, EndReason::Removed);
        }
        self.fire(Fired::new(Event::OnBundleDropped, caster));
    }

    // -------------------------------------------------------------- hooks

    /// A skill's energy cost after effects that change it (Soul Twisting).
    pub fn adjust_energy_cost(&self, unit: UnitId, skill: u16, cost: f64) -> f64 {
        let mut cost = cost;
        for effect in &self.units[unit.index()].effects {
            if let EffectSource::Handler { skill: owner } = effect.source
                && let Some(handler) = self.fight.skills[usize::from(owner)].handler
            {
                cost = self
                    .fight
                    .handlers
                    .get(handler)
                    .adjust_cost(self, unit, effect, skill, cost);
            }
        }
        cost
    }

    /// A skill's recharge after effects that change it (Soul Twisting).
    pub fn adjust_recharge(&self, unit: UnitId, skill: u16, recharge_ms: u32) -> u32 {
        let mut recharge = recharge_ms;
        for effect in &self.units[unit.index()].effects {
            if let EffectSource::Handler { skill: owner } = effect.source
                && let Some(handler) = self.fight.skills[usize::from(owner)].handler
            {
                recharge = self
                    .fight
                    .handlers
                    .get(handler)
                    .adjust_recharge(self, unit, effect, skill, recharge);
            }
        }
        recharge
    }

    /// A rank an effect sets outright (Master of Magic), if any.
    pub fn set_rank(&self, unit: UnitId, attribute: Attribute) -> Option<u8> {
        self.units[unit.index()]
            .effects
            .iter()
            .filter_map(|effect| match effect.source {
                EffectSource::Handler { skill } => self.fight.skills[usize::from(skill)]
                    .handler
                    .and_then(|h| self.fight.handlers.get(h).set_rank(self, effect, attribute)),
                _ => None,
            })
            .max()
    }

    /// Whether an effect prevents interrupting this unit. No M1 skill does.
    pub fn prevents_interrupt(&self, _unit: UnitId) -> bool {
        false
    }

    /// The title rank a track is at for this fight.
    pub fn title_rank(&self, track: TitleTrack) -> u8 {
        self.fight.title_rank(track)
    }
}
