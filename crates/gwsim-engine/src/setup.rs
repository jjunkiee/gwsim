//! Building a fight from data: units, skills, controllers (WP3.3, T3.10.2).
//!
//! [`FightSetup::new`] does everything that is the same for every run of a
//! party in a situation — resolving skills, spawning units with the WP1.4
//! derived stats, resolving plans — once. Each run then clones the prepared
//! state and gives it its own seed (§10.14).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use gwsim_data::assumptions::AssumptionValue;
use gwsim_data::build::{Build, SlotKind};
use gwsim_data::core::{CoreData, DamageType, Profession, SkillType, TitleTrack};
use gwsim_data::dataset::DataSet;
use gwsim_data::derived::{self, PerDamageType};
use gwsim_data::dsl::{ModCategory, Stat};
use gwsim_data::foe::{CreatureTrait, Dummy, Foe, SkillRef};
use gwsim_data::party::{PartyFile, PartySlot};
use gwsim_data::plan::PriorityPlan;
use gwsim_data::provenance::ReviewStatus;
use gwsim_data::scenario::{Encounter, Formation, Situation, SituationEncounters};
use gwsim_data::skill::Skill;
use gwsim_data::{AssumptionId, Slug};

use crate::ai::{Controller, ResolvedPlan, ResolvedRule};
use crate::geom::Vec2;
use crate::handlers::HandlerRegistry;
use crate::result::RunResult;
use crate::rng::RunSeed;
use crate::sim::Sim;
use crate::stats::{GearEffect, ModSource, Modifier};
use crate::time::SimTime;
use crate::unit::{
    Action, ENERGY_SCALE, HEALTH_SCALE, SlotState, Team, Unit, UnitId, UnitKind, WeaponProfile,
};

/// A skill as a fight uses it.
#[derive(Debug, Clone)]
pub struct FightSkill {
    pub skill: Skill,
    pub slug: Slug,
    pub handler: Option<usize>,
    /// What the skill is for, as the AI reads it.
    pub profile: crate::ai::profile::SkillProfile,
    pub aftercast_ms: u32,
    pub is_spell: bool,
    pub is_attack: bool,
    /// Used while only `Draft`, so results must say so (§8.7).
    pub draft: bool,
    /// Per effect definition, the stats its `while_active` can modify: every
    /// one, and those it gives its caster (T4.11.4).
    pub def_stats: Vec<(u64, u64)>,
    /// Per effect definition, what else it holds ([`DEF_REDUCES`] and so on),
    /// so lookups skip definitions that cannot matter.
    pub def_flags: Vec<u8>,
}

/// A definition reduces incoming damage.
pub const DEF_REDUCES: u8 = 1;
/// A definition grants immunity to critical hits.
pub const DEF_CRIT_IMMUNE: u8 = 2;
/// A definition has triggers.
pub const DEF_TRIGGERS: u8 = 4;

impl FightSkill {
    /// Prepares a skill for a fight.
    pub fn new(skill: Skill, slug: Slug, handlers: &HandlerRegistry) -> FightSkill {
        let kind = skill.kind;
        let is_attack = kind.is_a(SkillType::AttackSkill);
        // Attack skills recover through the attack cycle rather than a flat
        // aftercast (Aftercast delay: their animation is part of the attack).
        let aftercast_ms = if is_attack {
            0
        } else {
            kind.default_aftercast_ms()
        };
        let handler = skill
            .encoding
            .as_ref()
            .and_then(|e| e.handler.as_ref())
            .and_then(|h| handlers.index_of(&h.name));
        let def_stats = skill
            .encoding
            .as_ref()
            .map(|e| {
                e.effect_defs
                    .iter()
                    .map(|d| crate::stats::actions_mask(&d.while_active))
                    .collect()
            })
            .unwrap_or_default();
        let def_flags = skill
            .encoding
            .as_ref()
            .map(|e| {
                e.effect_defs
                    .iter()
                    .map(|d| {
                        let mut flags = 0;
                        for action in &d.while_active {
                            match action {
                                gwsim_data::dsl::Action::ReduceIncomingDamage { .. } => {
                                    flags |= DEF_REDUCES;
                                }
                                gwsim_data::dsl::Action::SetCriticalImmune { .. } => {
                                    flags |= DEF_CRIT_IMMUNE;
                                }
                                _ => {}
                            }
                        }
                        if !d.triggers.is_empty() {
                            flags |= DEF_TRIGGERS;
                        }
                        flags
                    })
                    .collect()
            })
            .unwrap_or_default();
        FightSkill {
            def_stats,
            def_flags,
            profile: crate::ai::profile::SkillProfile::of(&skill),
            draft: skill.provenance.review == ReviewStatus::Draft,
            is_spell: kind.is_a(SkillType::Spell),
            is_attack,
            aftercast_ms,
            handler,
            skill,
            slug,
        }
    }
}

/// Values the engine reads from the assumptions register (ENG-4).
#[derive(Debug, Clone, PartialEq)]
pub struct Tunables {
    /// A-001.
    pub base_speed: f32,
    /// A-002.
    pub collision_radius: f32,
    /// A-003, by weapon slug.
    pub projectile_speeds: BTreeMap<String, f32>,
    /// A-009.
    pub aggro_range: f32,
    /// A-010, in normal mode.
    pub foe_reaction_ms: u32,
    /// A-010, in hard mode.
    pub foe_reaction_hm_ms: u32,
    /// A-011, for decisions other than interrupts.
    pub hero_reaction_ms: u32,
    /// A-012.
    pub human_reaction_ms: u32,
    /// A-028.
    pub rest_ms: u32,
    /// A-030.
    pub timeout_ms: u32,
    /// A-032, when settled.
    pub hard_mode_recharge_reduction: Option<f64>,
    /// A-035.
    pub attack_hit_fraction: f64,
    /// A-036.
    pub experience_range: f32,
}

impl Tunables {
    /// Reads the register. A value the engine cannot run without that is
    /// still `Pending` is an error naming it.
    pub fn from_data(data: &DataSet) -> Result<Tunables, SetupError> {
        let mut problems = Vec::new();
        let get = |id: &str| {
            data.assumptions
                .get(id.parse::<AssumptionId>().ok()?)
                .map(|a| &a.value)
        };
        let number = |id: &str, problems: &mut Vec<String>| -> f64 {
            match get(id) {
                Some(AssumptionValue::Number(n)) => *n,
                Some(AssumptionValue::Millis(ms)) => f64::from(*ms),
                Some(AssumptionValue::Gwinches(g)) => f64::from(*g),
                Some(AssumptionValue::Percent(p)) => f64::from(*p),
                Some(AssumptionValue::Pending) => {
                    problems.push(format!(
                        "{id} is still Pending, and the engine needs its value"
                    ));
                    0.0
                }
                _ => {
                    problems.push(format!("{id} has no numeric value"));
                    0.0
                }
            }
        };
        let optional = |id: &str| -> Option<f64> {
            match get(id)? {
                AssumptionValue::Number(n) => Some(*n),
                AssumptionValue::Millis(ms) => Some(f64::from(*ms)),
                AssumptionValue::Percent(p) => Some(f64::from(*p)),
                _ => None,
            }
        };
        let table_entry = |id: &str, key: &str| -> Option<f64> {
            match get(id)? {
                AssumptionValue::Table(rows) => {
                    rows.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
                }
                _ => None,
            }
        };
        let projectile_speeds = match get("A-003") {
            Some(AssumptionValue::Table(rows)) => {
                rows.iter().map(|(k, v)| (k.clone(), *v as f32)).collect()
            }
            _ => {
                problems.push("A-003 needs a table of projectile speeds".to_owned());
                BTreeMap::new()
            }
        };
        let human = number("A-012", &mut problems) as u32;
        let tunables = Tunables {
            base_speed: number("A-001", &mut problems) as f32,
            collision_radius: number("A-002", &mut problems) as f32,
            projectile_speeds,
            aggro_range: number("A-009", &mut problems) as f32,
            foe_reaction_ms: table_entry("A-010", "normal").unwrap_or(f64::from(human)) as u32,
            foe_reaction_hm_ms: table_entry("A-010", "hard").unwrap_or(f64::from(human)) as u32,
            hero_reaction_ms: optional("A-011").map(|v| v as u32).unwrap_or(human),
            human_reaction_ms: human,
            rest_ms: number("A-028", &mut problems) as u32,
            timeout_ms: number("A-030", &mut problems) as u32,
            hard_mode_recharge_reduction: optional("A-032"),
            attack_hit_fraction: number("A-035", &mut problems) / 100.0,
            experience_range: number("A-036", &mut problems) as f32,
        };
        if problems.is_empty() {
            Ok(tunables)
        } else {
            Err(SetupError(problems))
        }
    }
}

/// Everything shared, read-only, by every run of one setup.
#[derive(Debug)]
pub struct FightData {
    pub core: CoreData,
    pub skills: Vec<FightSkill>,
    pub tunables: Tunables,
    pub handlers: HandlerRegistry,
    pub plans: Vec<ResolvedPlan>,
    pub hard_mode: bool,
    pub timeout_ms: u32,
    /// The spirits skills can create, by slug (`spirits.ron`, A-015).
    pub spirits: BTreeMap<String, gwsim_data::foe::Spirit>,
    /// The minions skills can create, by slug (`minions.ron`).
    pub minions: BTreeMap<String, gwsim_data::foe::Minion>,
    /// Weapon profiles for minions, by slug, resolved once.
    pub minion_weapons: BTreeMap<String, WeaponProfile>,
    /// Death penalty the party starts with, and its morale boost (§10.11).
    pub starting_dp: u8,
    pub starting_morale: u8,
    /// Whether Dhuum's Covenant is on (a party death breaks it).
    pub dhuums_covenant: bool,
    /// The stats any effect in the fight gives its caster rather than its
    /// bearer; queries for others skip the caster scan.
    pub caster_share_mask: u64,
    /// Whether any skill in the fight maintains an effect with upkeep.
    pub any_upkeep: bool,
    /// Title ranks for PvE-only skills. With no account profile every track
    /// is at its maximum (Q14).
    pub title_ranks: BTreeMap<TitleTrack, u8>,
}

impl FightData {
    /// The rank a title track is at.
    pub fn title_rank(&self, track: TitleTrack) -> u8 {
        self.title_ranks
            .get(&track)
            .copied()
            .or_else(|| self.core.title_track(track).map(|t| t.max_rank))
            .unwrap_or(0)
    }
}

/// Why a fight could not be set up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupError(pub Vec<String>);

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for problem in &self.0 {
            writeln!(f, "{problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for SetupError {}

/// A party in a situation, prepared once and run many times.
#[derive(Debug, Clone)]
pub struct FightSetup {
    pub fight: Arc<FightData>,
    units: Vec<Unit>,
    controllers: Vec<Controller>,
    /// The party's slot names, in order.
    pub slot_names: Vec<String>,
    /// The foes' names, in order.
    pub foe_names: Vec<String>,
}

/// Skills resolved for one setup.
struct SkillTable<'a> {
    data: &'a DataSet,
    handlers: &'a HandlerRegistry,
    skills: Vec<FightSkill>,
    by_slug: BTreeMap<Slug, u16>,
}

impl SkillTable<'_> {
    fn add(&mut self, slug: &Slug) -> Result<u16, String> {
        if let Some(index) = self.by_slug.get(slug) {
            return Ok(*index);
        }
        let skill = self
            .data
            .skill(slug)
            .ok_or_else(|| format!("no skill {slug} in the data"))?;
        if skill.provenance.review == ReviewStatus::NumbersOnly {
            return Err(format!(
                "{} is NumbersOnly: it has no encoding, and the evaluator refuses builds that use one (§8.7)",
                skill.name
            ));
        }
        let index = self.skills.len() as u16;
        self.skills
            .push(FightSkill::new(skill.clone(), slug.clone(), self.handlers));
        self.by_slug.insert(slug.clone(), index);
        Ok(index)
    }

    fn add_ref(&mut self, reference: &SkillRef) -> Result<u16, String> {
        let slug = match reference {
            SkillRef::Slug(slug) => slug.clone(),
            SkillRef::Id(id) => self
                .data
                .skill_slug_for_id(*id)
                .cloned()
                .ok_or_else(|| format!("no skill with id {id}"))?,
        };
        self.add(&slug)
    }
}

impl FightSetup {
    /// Prepares a party for a situation.
    pub fn new(
        data: &DataSet,
        core: &CoreData,
        party: &PartyFile,
        situation: &Situation,
    ) -> Result<FightSetup, SetupError> {
        FightSetup::with_handlers(data, core, party, situation, HandlerRegistry::standard())
    }

    /// Prepares a fight with a given handler registry (tests use stand-ins).
    pub fn with_handlers(
        data: &DataSet,
        core: &CoreData,
        party: &PartyFile,
        situation: &Situation,
        handlers: HandlerRegistry,
    ) -> Result<FightSetup, SetupError> {
        let tunables = Tunables::from_data(data)?;
        let hard_mode = situation.mode.hard_mode;
        let timeout_ms = situation
            .timeout
            .map(|t| t.ms())
            .unwrap_or(tunables.timeout_ms);
        let mut problems = Vec::new();
        let mut table = SkillTable {
            data,
            handlers: &handlers,
            skills: Vec::new(),
            by_slug: BTreeMap::new(),
        };

        let encounter = match &situation.encounters {
            SituationEncounters::Single(slug) => data.encounters.get(slug).map(|e| &e.value),
            SituationEncounters::Chain(steps) => steps
                .first()
                .and_then(|step| data.encounters.get(&step.encounter))
                .map(|e| &e.value),
        };
        let Some(encounter) = encounter else {
            return Err(SetupError(vec![format!(
                "situation {:?} names no known encounter",
                situation.name
            )]));
        };

        let mut units = Vec::new();
        let mut controllers = Vec::new();
        let mut plans = Vec::new();
        let start = Vec2::new(encounter.party_start.x, encounter.party_start.y);

        for (index, slot) in party.slots.iter().enumerate() {
            let id = UnitId(units.len() as u16);
            match party_unit(id, index, slot, data, core, &mut table, &tunables, start) {
                Ok((unit, bar_refs)) => {
                    let controller = match slot.kind {
                        SlotKind::Human => {
                            let plan = slot
                                .plan
                                .clone()
                                .unwrap_or_else(|| default_plan(&slot.build));
                            match resolve_plan(&plan, &unit, &bar_refs, &mut table) {
                                Ok(resolved) => {
                                    plans.push(resolved);
                                    Controller::Plan {
                                        plan: (plans.len() - 1) as u16,
                                    }
                                }
                                Err(problem) => {
                                    problems.push(format!("{}: {problem}", slot.name));
                                    Controller::Idle
                                }
                            }
                        }
                        SlotKind::Hero | SlotKind::Henchman => Controller::Hero,
                    };
                    units.push(unit);
                    controllers.push(controller);
                }
                Err(problem) => problems.push(format!("{}: {problem}", slot.name)),
            }
        }

        let mut foe_names = Vec::new();
        spawn_encounter(
            encounter,
            data,
            core,
            hard_mode,
            &tunables,
            &mut table,
            &mut units,
            &mut controllers,
            &mut foe_names,
            &mut problems,
        );

        if !problems.is_empty() {
            return Err(SetupError(problems));
        }

        let spirits: BTreeMap<String, gwsim_data::foe::Spirit> = data
            .spirits
            .as_ref()
            .map(|file| {
                file.value
                    .spirits
                    .iter()
                    .map(|s| (s.slug.to_string(), s.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let minions: BTreeMap<String, gwsim_data::foe::Minion> = data
            .minions
            .as_ref()
            .map(|file| {
                file.value
                    .minions
                    .iter()
                    .map(|m| (m.slug.to_string(), m.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let minion_weapons = minions
            .iter()
            .filter_map(|(slug, minion)| {
                let weapon = minion.weapon.as_ref()?;
                let mut profile = weapon_profile(
                    data,
                    &weapon.weapon_type,
                    Profession::Necromancer,
                    &tunables,
                )?;
                if let Some((low, high)) = weapon.damage {
                    profile.damage = (i32::from(low), i32::from(high));
                }
                if let Some(interval) = weapon.attack_interval {
                    profile.interval_ms = interval.ms();
                }
                if let Some(range) = minion.range {
                    profile.range = range;
                }
                // A-043: minions strike at three times their level.
                profile.mastery = None;
                Some((slug.clone(), profile))
            })
            .collect();
        for unit in &mut units {
            if unit.team == Team::Party && !unit.kind.is_summoned() {
                unit.death_penalty = situation.starting_dp;
                unit.morale = situation.starting_morale;
            }
        }
        let caster_share_mask = table
            .skills
            .iter()
            .flat_map(|s| s.def_stats.iter().map(|(_, caster)| *caster))
            .fold(0, |mask, bits| mask | bits);
        let any_upkeep = table.skills.iter().any(|s| {
            s.skill
                .encoding
                .as_ref()
                .is_some_and(|e| e.effect_defs.iter().any(|d| d.upkeep.is_some()))
        });
        let fight = FightData {
            core: core.clone(),
            skills: table.skills,
            tunables,
            handlers,
            plans,
            hard_mode,
            timeout_ms,
            spirits,
            minions,
            minion_weapons,
            starting_dp: situation.starting_dp,
            starting_morale: situation.starting_morale,
            dhuums_covenant: situation.mode.dhuums_covenant,
            caster_share_mask,
            any_upkeep,
            title_ranks: BTreeMap::new(),
        };
        let slot_names = party.slots.iter().map(|s| s.name.clone()).collect();
        Ok(FightSetup {
            fight: Arc::new(fight),
            units,
            controllers,
            slot_names,
            foe_names,
        })
    }

    /// A fresh fight for a seed.
    pub fn sim(&self, seed: RunSeed) -> Sim {
        Sim::new(
            Arc::clone(&self.fight),
            self.units.clone(),
            self.controllers.clone(),
            seed,
        )
    }

    /// Runs one seed.
    pub fn run(&self, seed: RunSeed) -> RunResult {
        self.sim(seed).run()
    }

    /// Runs one seed with the combat log on (T3.9.4).
    pub fn run_logged(&self, seed: RunSeed) -> RunResult {
        self.sim(seed).with_log().run()
    }

    /// The prepared units, before any run.
    pub fn units(&self) -> &[Unit] {
        &self.units
    }

    /// A unit's name, for logs.
    pub fn unit_name(&self, id: u16) -> String {
        self.units
            .get(usize::from(id))
            .map(|u| u.name.clone())
            .unwrap_or_else(|| format!("unit {id}"))
    }

    /// A skill's name by template id, for logs.
    pub fn skill_name(&self, id: u16) -> String {
        self.fight
            .skills
            .iter()
            .find(|s| s.skill.id.get() == id)
            .map(|s| s.skill.name.clone())
            .unwrap_or_else(|| format!("skill {id}"))
    }
}

/// A plan for a human slot with none written: every skill in bar order at
/// the called or current target. WP4.6 replaces this with a generator.
fn default_plan(build: &Build) -> PriorityPlan {
    PriorityPlan {
        maintain: Vec::new(),
        rules: build
            .skills
            .iter()
            .flatten()
            .map(|id| gwsim_data::plan::PlanRule {
                skill: SkillRef::Id(*id),
                target: gwsim_data::plan::PlanTarget::CalledOrCurrent,
                only_if: Vec::new(),
            })
            .collect(),
        default: gwsim_data::plan::DefaultAction::Attack,
    }
}

fn resolve_plan(
    plan: &PriorityPlan,
    unit: &Unit,
    bar_refs: &[Option<u16>; 8],
    table: &mut SkillTable<'_>,
) -> Result<ResolvedPlan, String> {
    // Rules name skills, not slots: a slot's skill can change mid-fight
    // (Arcane Echo), and a copy should be used like the original.
    let slot_of = |reference: &SkillRef, table: &mut SkillTable<'_>| -> Result<u16, String> {
        let index = table.add_ref(reference)?;
        if bar_refs.contains(&Some(index)) {
            Ok(index)
        } else {
            Err(format!(
                "the plan names {reference:?}, which is not on {}'s bar",
                unit.name
            ))
        }
    };
    let mut resolved = ResolvedPlan {
        default: plan.default,
        ..ResolvedPlan::default()
    };
    for reference in &plan.maintain {
        resolved.maintain.push(slot_of(reference, table)?);
    }
    for rule in &plan.rules {
        resolved.rules.push(ResolvedRule {
            skill: slot_of(&rule.skill, table)?,
            target: rule.target.clone(),
            only_if: rule.only_if.clone(),
        });
    }
    Ok(resolved)
}

/// An empty unit with the fields every kind shares.
pub(crate) fn blank_unit(
    id: UnitId,
    name: String,
    team: Team,
    kind: UnitKind,
    level: u8,
    radius: f32,
) -> Unit {
    Unit {
        id,
        name,
        team,
        kind,
        level,
        professions: (Profession::Warrior, None),
        species: "Human".to_owned(),
        traits: vec![CreatureTrait::Fleshy],
        master: None,
        slot_index: None,
        foe_index: None,
        gives_experience: false,
        stationary: false,
        natural_regeneration: true,
        pos: Vec2::ZERO,
        velocity: Vec2::ZERO,
        facing: Vec2::new(0.0, 1.0),
        radius,
        goal: None,
        health: 0,
        base_max_health: 1,
        energy: 0,
        base_max_energy: 0,
        overcast: 0,
        base_energy_pips: 0,
        base_health_pips: 0,
        bar: [None; 8],
        action: Action::Idle,
        action_generation: 0,
        attack_target: None,
        next_swing_at: SimTime::ZERO,
        swing_hits_at: None,
        effects: Vec::new(),
        base_ranks: [0; 42],
        armor: [PerDamageType {
            base: 0,
            by_type: [0; 11],
        }; 5],
        weapon: None,
        permanent: Vec::new(),
        gear: Vec::new(),
        last_combat: None,
        next_decision_at: SimTime::ZERO,
        dead_at: None,
        soul_reaping: Vec::new(),
        corpse_available: false,
        born_at: SimTime::ZERO,
        aura: None,
        creature_type: None,
        death_penalty: 0,
        morale: 0,
        group: None,
        hero_mode: crate::unit::HeroMode::Fight,
        focus: None,
        home: Vec2::ZERO,
        kiter: false,
    }
}

fn slot_state(skill: u16) -> SlotState {
    SlotState {
        skill,
        original: skill,
        ready_at: SimTime::ZERO,
        disabled_until: SimTime::ZERO,
        adrenaline: 0,
        revert_generation: 0,
    }
}

/// A weapon from `weapons.ron`, resolved for the engine.
fn weapon_profile(
    data: &DataSet,
    slug: &Slug,
    wielder: Profession,
    tunables: &Tunables,
) -> Option<WeaponProfile> {
    let weapon = data
        .weapons
        .as_ref()?
        .value
        .weapons
        .iter()
        .find(|w| w.slug == *slug)?;
    let (low, high) = weapon.damage_range_at_max_req?;
    let caster = weapon.mastery.is_none();
    let damage_type = weapon
        .damage_type
        .unwrap_or_else(|| Sim::caster_damage_type(wielder));
    Some(WeaponProfile {
        slug: slug.to_string(),
        damage: (i32::from(low), i32::from(high)),
        damage_type,
        interval_ms: weapon.attack_interval.map(|s| s.ms()).unwrap_or(1750),
        range: weapon.range.unwrap_or(144.0),
        projectile_speed: weapon
            .projectile_speed
            .or_else(|| tunables.projectile_speeds.get(slug.as_str()).copied()),
        mastery: weapon.mastery,
        caster,
        armor_ignoring: false,
    })
}

#[allow(clippy::too_many_arguments)]
fn party_unit(
    id: UnitId,
    slot_index: usize,
    slot: &PartySlot,
    data: &DataSet,
    core: &CoreData,
    table: &mut SkillTable<'_>,
    tunables: &Tunables,
    start: Vec2,
) -> Result<(Unit, [Option<u16>; 8]), String> {
    let build = &slot.build;
    let level = 20;
    let mut unit = blank_unit(
        id,
        slot.name.clone(),
        Team::Party,
        UnitKind::Slot(slot.kind),
        level,
        tunables.collision_radius,
    );
    unit.slot_index = Some(slot_index);
    unit.professions = (build.primary, build.secondary);
    unit.pos = start + Vec2::new(0.0, -(slot_index as f32) * 60.0);
    // A hero's formation point, relative to the leader (the tactics plan
    // replaces this, WP4.7).
    unit.home = unit.pos - start;

    for (attribute, rank) in derived::effective_ranks(build, data) {
        unit.base_ranks[attribute.index()] = rank;
    }
    unit.base_max_health = derived::max_health(build, level, data, core);
    unit.health = unit.base_max_health * HEALTH_SCALE;
    let energy = derived::energy(build, data, core);
    unit.base_max_energy = energy.max;
    unit.energy = energy.max * ENERGY_SCALE;
    unit.base_energy_pips = i32::from(energy.regen_pips);
    unit.armor = derived::armor_profile(build, core).pieces;

    if let Some(main) = &build.weapon_set.main {
        unit.weapon = weapon_profile(data, main, build.primary, tunables);
    }

    // Insignias: per-piece ones apply to their own piece's armor only.
    if let Some(insignias) = &data.insignias {
        for piece in &build.armor {
            let Some(slug) = &piece.insignia else {
                continue;
            };
            if let Some(insignia) = insignias.value.insignias.iter().find(|i| i.slug == *slug) {
                unit.gear.push(GearEffect {
                    name: insignia.name.clone(),
                    actions: insignia.effects.clone(),
                    piece: insignia.per_piece.then_some(piece.slot),
                    scope: None,
                    stats: crate::stats::actions_mask(&insignia.effects).0,
                });
            }
        }
    }
    // Weapon upgrades apply to the wielder.
    if let Some(upgrades) = &data.weapon_upgrades {
        let set = &build.weapon_set;
        let slugs = [&set.prefix, &set.suffix, &set.inscription]
            .into_iter()
            .flatten()
            .chain(set.offhand_upgrades.iter());
        for slug in slugs {
            if let Some(upgrade) = upgrades.value.upgrades.iter().find(|u| u.slug == *slug) {
                unit.gear.push(GearEffect {
                    name: upgrade.name.clone(),
                    actions: upgrade.effects.clone(),
                    piece: None,
                    scope: set.attribute,
                    stats: crate::stats::actions_mask(&upgrade.effects).0,
                });
            }
        }
    }

    let mut bar_refs = [None; 8];
    for (position, id) in build.skills.iter().enumerate() {
        let Some(id) = id else { continue };
        let index = table.add_ref(&SkillRef::Id(*id))?;
        unit.bar[position] = Some(slot_state(index));
        bar_refs[position] = Some(index);
    }
    Ok((unit, bar_refs))
}

#[allow(clippy::too_many_arguments)]
fn spawn_encounter(
    encounter: &Encounter,
    data: &DataSet,
    core: &CoreData,
    hard_mode: bool,
    tunables: &Tunables,
    table: &mut SkillTable<'_>,
    units: &mut Vec<Unit>,
    controllers: &mut Vec<Controller>,
    names: &mut Vec<String>,
    problems: &mut Vec<String>,
) {
    let start = Vec2::new(encounter.party_start.x, encounter.party_start.y);
    let radius = tunables.collision_radius;
    for (group_index, group) in encounter.groups.iter().enumerate() {
        let centre = start + Vec2::new(group.position.x, group.position.y);
        let members: Vec<(&Slug, Option<&String>)> = group
            .foes
            .iter()
            .flat_map(|f| std::iter::repeat_n((&f.foe, f.variant.as_ref()), usize::from(f.count)))
            .collect();
        let offsets = layout(&group.formation, members.len());
        for (index, (slug, variant)) in members.into_iter().enumerate() {
            let id = UnitId(units.len() as u16);
            let position = centre + offsets[index];
            let spawned = if let Some(dummy) = data.dummy(slug) {
                Ok((dummy_unit(id, dummy, radius), Controller::Idle))
            } else if let Some(foe) = data.foe(slug) {
                let level = group
                    .level_override
                    .as_ref()
                    .map(|l| *l.get(hard_mode))
                    .or_else(|| derived::foe_level(foe, hard_mode));
                foe_unit(
                    id, foe, variant, level, hard_mode, data, core, table, tunables,
                )
                .map(|unit| (unit, Controller::Foe))
            } else {
                Err(format!(
                    "encounter {:?} names {slug}, which is neither a foe nor a dummy",
                    encounter.name
                ))
            };
            match spawned {
                Ok((mut unit, controller)) => {
                    unit.pos = position;
                    unit.home = position;
                    unit.group = Some(group_index as u16);
                    unit.foe_index = Some(names.len());
                    names.push(unit.name.clone());
                    units.push(unit);
                    controllers.push(controller);
                }
                Err(problem) => problems.push(problem),
            }
        }
    }
}

/// Where each member of a group stands, relative to the group's position.
///
/// `Cluster` places members on a golden-angle spiral inside its radius:
/// deterministic, evenly spread, and the same for every seed (A-008 already
/// makes the geometry an assumption).
pub fn layout(formation: &Formation, count: usize) -> Vec<Vec2> {
    match formation {
        Formation::Explicit(positions) => (0..count)
            .map(|i| {
                positions
                    .get(i)
                    .map(|p| Vec2::new(p.x, p.y))
                    .unwrap_or(Vec2::ZERO)
            })
            .collect(),
        Formation::Line { spacing } => {
            let width = (count.saturating_sub(1)) as f32 * spacing;
            (0..count)
                .map(|i| Vec2::new(i as f32 * spacing - width / 2.0, 0.0))
                .collect()
        }
        Formation::Cluster { radius } => {
            const GOLDEN_ANGLE: f32 = 2.399_963;
            (0..count)
                .map(|i| {
                    if count == 1 {
                        return Vec2::ZERO;
                    }
                    let r = radius * ((i as f32 + 0.5) / count as f32).sqrt();
                    let angle = i as f32 * GOLDEN_ANGLE;
                    // A rational approximation keeps platform trigonometry
                    // out of the result.
                    Vec2::new(r * approx_cos(angle), r * approx_sin(angle))
                })
                .collect()
        }
    }
}

/// `cos` by a fixed Taylor series after range reduction, so layouts never
/// depend on the platform's maths library.
fn approx_cos(angle: f32) -> f32 {
    approx_sin(angle + std::f32::consts::FRAC_PI_2)
}

fn approx_sin(angle: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let mut x = angle % tau;
    if x > std::f32::consts::PI {
        x -= tau;
    }
    if x < -std::f32::consts::PI {
        x += tau;
    }
    // sin(π − x) = sin(x): fold into [−π/2, π/2], where the series is tight.
    if x > std::f32::consts::FRAC_PI_2 {
        x = std::f32::consts::PI - x;
    } else if x < -std::f32::consts::FRAC_PI_2 {
        x = -std::f32::consts::PI - x;
    }
    let x2 = x * x;
    x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0))))
}

fn dummy_unit(id: UnitId, dummy: &Dummy, radius: f32) -> Unit {
    let mut unit = blank_unit(
        id,
        dummy.name.clone(),
        Team::Foes,
        UnitKind::Dummy,
        dummy.level,
        radius,
    );
    unit.stationary = true;
    unit.natural_regeneration = false;
    unit.gives_experience = dummy.gives_experience;
    unit.traits = dummy.traits.clone();
    unit.species = "Dummy".to_owned();
    unit.base_max_health = dummy.health as i32;
    unit.health = unit.base_max_health * HEALTH_SCALE;
    unit.base_max_energy = dummy.energy as i32;
    unit.energy = unit.base_max_energy * ENERGY_SCALE;
    unit.base_energy_pips = i32::from(dummy.energy_regeneration);
    let armor = PerDamageType {
        base: dummy.armor,
        by_type: [dummy.armor; 11],
    };
    unit.armor = [armor; 5];
    unit
}

#[allow(clippy::too_many_arguments)]
fn foe_unit(
    id: UnitId,
    foe: &Foe,
    variant: Option<&String>,
    level: Option<u8>,
    hard_mode: bool,
    data: &DataSet,
    core: &CoreData,
    table: &mut SkillTable<'_>,
    tunables: &Tunables,
) -> Result<Unit, String> {
    let level = level.ok_or_else(|| format!("{} has no level for this mode", foe.name))?;
    let mut unit = blank_unit(
        id,
        foe.name.clone(),
        Team::Foes,
        UnitKind::Foe,
        level,
        tunables.collision_radius,
    );
    unit.professions = foe.professions;
    unit.species = foe.species.clone();
    unit.traits = foe.traits.clone();
    unit.gives_experience = true;
    unit.base_max_health = foe
        .health_override
        .map(|h| h as i32)
        .unwrap_or_else(|| derived::foe_max_health(core, level, hard_mode) as i32);
    unit.health = unit.base_max_health * HEALTH_SCALE;
    let energy = derived::foe_energy(core, foe.professions.0);
    unit.base_max_energy = foe.energy.map(|e| e as i32).unwrap_or(energy.max);
    unit.energy = unit.base_max_energy * ENERGY_SCALE;
    unit.base_energy_pips = i32::from(energy.regen_pips);
    let (ranks, _) = derived::foe_attributes(foe, core, hard_mode);
    for (attribute, rank) in ranks {
        unit.base_ranks[attribute.index()] = rank;
    }
    let chosen = variant.and_then(|name| foe.variants.iter().find(|v| v.name == *name));
    let armor_table = chosen.and_then(|v| v.armor.as_ref()).unwrap_or(&foe.armor);
    let mut by_type = [0i16; 11];
    for damage in DamageType::ALL {
        by_type[damage.index()] =
            derived::foe_armor_with(foe, armor_table, core, damage, hard_mode);
    }
    // Chaos is the type no profession bonus touches, so it is the base.
    let base = by_type[DamageType::Chaos.index()];
    unit.armor = [PerDamageType { base, by_type }; 5];

    let weapon = chosen
        .and_then(|v| v.weapon.as_ref())
        .or(foe.weapon.as_ref());
    if let Some(weapon) = weapon
        && let Some(mut profile) =
            weapon_profile(data, &weapon.weapon_type, foe.professions.0, tunables)
    {
        if let Some((low, high)) = weapon.damage {
            profile.damage = (i32::from(low), i32::from(high));
        }
        if let Some(interval) = weapon.attack_interval {
            profile.interval_ms = interval.ms();
        }
        unit.weapon = Some(profile);
    }

    let bar = chosen
        .and_then(|v| v.skills.as_ref())
        .unwrap_or(&foe.skills);
    let mut position = 0;
    for foe_skill in bar {
        if foe_skill.hm_only && !hard_mode {
            continue;
        }
        if position >= 8 {
            break;
        }
        let index = table.add_ref(&foe_skill.skill)?;
        unit.bar[position] = Some(slot_state(index));
        position += 1;
    }

    if hard_mode {
        let rules = &core.modes.hard_mode;
        for (stat, value) in [
            (
                Stat::MovementSpeed,
                f64::from(rules.movement_speed_bonus_percent),
            ),
            (
                Stat::AttackSpeed,
                f64::from(rules.attack_speed_bonus_percent),
            ),
        ] {
            unit.permanent.push(Modifier {
                stat,
                value,
                category: ModCategory::Multiplicative,
                exceeds_cap: false,
                source: ModSource::HardMode,
            });
        }
    }
    Ok(unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_approximate_sine_is_close_enough_for_layouts() {
        for step in 0..100 {
            let angle = step as f32 * 0.37;
            assert!((approx_sin(angle) - angle.sin()).abs() < 1e-3, "{angle}");
        }
    }

    #[test]
    fn a_cluster_stays_inside_its_radius_and_is_deterministic() {
        let formation = Formation::Cluster { radius: 150.0 };
        let first = layout(&formation, 8);
        assert_eq!(first, layout(&formation, 8));
        assert!(first.iter().all(|p| p.length() <= 150.0 + 1e-3));
    }
}
