//! Units: everything on the field that has health (T3.3.1, DESIGN §10.4).
//!
//! Units live in one flat `Vec` in the fight, indexed by [`UnitId`], in a
//! stable order: party slots first, then foes in encounter order, then
//! anything created mid-fight. That order is also what the random streams
//! are keyed on (T3.7.2), so it must never depend on anything but the
//! inputs.
//!
//! Health and energy are held in fixed point so that regeneration, which is
//! applied on every 50 ms tick, adds up exactly: one health pip over one
//! tick is 0.1 HP, which is 100 milli-HP, and one energy pip over one tick
//! is 1/60 energy, which is 50 units of 1/3000 energy.

use gwsim_data::build::SlotKind;
use gwsim_data::core::{Attribute, DamageType, Profession};
use gwsim_data::derived::PerDamageType;
use gwsim_data::foe::CreatureTrait;

use crate::effects::ActiveEffect;
use crate::geom::Vec2;
use crate::time::SimTime;

/// Health is stored in thousandths of a point.
pub const HEALTH_SCALE: i32 = 1000;
/// Energy is stored in three-thousandths of a point, so that one pip over one
/// tick is a whole number of units.
pub const ENERGY_SCALE: i32 = 3000;
/// Adrenaline is stored in the game's own units: 25 to a strike.
pub const ADRENALINE_PER_STRIKE: i32 = 25;
/// How many skills a bar holds.
pub const BAR: usize = 8;

/// A unit's index in the fight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct UnitId(pub u16);

impl UnitId {
    /// As an index into the unit list.
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// Which side a unit fights on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Team {
    Party,
    Foes,
}

impl Team {
    /// The other side.
    pub fn other(self) -> Team {
        match self {
            Team::Party => Team::Foes,
            Team::Foes => Team::Party,
        }
    }
}

/// What sort of unit this is, which decides its controller and a few rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitKind {
    /// A party slot: the player, a hero or a henchman.
    Slot(SlotKind),
    /// A foe from the encounter.
    Foe,
    /// A training dummy: stationary, and does nothing.
    Dummy,
    /// A spirit a ritual created.
    Spirit,
    /// A minion an Animate skill created.
    Minion,
}

impl UnitKind {
    /// Whether this unit was created by a skill.
    pub fn is_summoned(self) -> bool {
        matches!(self, UnitKind::Spirit | UnitKind::Minion)
    }
}

/// What a unit is doing with its body right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Idle,
    /// Activating a skill in `slot`, finishing at `ends_at`.
    Activating {
        slot: u8,
        target: Target,
        started: SimTime,
        ends_at: SimTime,
        /// Set by a `FailSkill` while the skill is still activating.
        failed: bool,
    },
    /// Recovering after a skill.
    Aftercast {
        ends_at: SimTime,
    },
    /// Attacking a unit with the weapon.
    Attacking {
        target: UnitId,
    },
    KnockedDown {
        until: SimTime,
    },
    Dead,
}

/// What a skill or order is aimed at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    None,
    Unit(UnitId),
    Location(Vec2),
}

impl Eq for Target {}

impl Target {
    /// The unit, if the target is one.
    pub fn unit(self) -> Option<UnitId> {
        match self {
            Target::Unit(unit) => Some(unit),
            _ => None,
        }
    }
}

/// One skill slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotState {
    /// The skill in the slot now, as an index into the fight's skill table.
    pub skill: u16,
    /// The skill that belongs in the slot, when a copy has replaced it.
    pub original: u16,
    /// When the slot can next be used.
    pub ready_at: SimTime,
    /// Disabled until then; recharge modifiers do not touch it.
    pub disabled_until: SimTime,
    /// Adrenaline, in units of 1/25 strike.
    pub adrenaline: i32,
    /// Bumped whenever a revert is scheduled, so a stale one is skipped.
    pub revert_generation: u32,
}

/// A weapon, resolved for the engine.
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponProfile {
    pub slug: String,
    pub damage: (i32, i32),
    pub damage_type: DamageType,
    pub interval_ms: u32,
    pub range: f32,
    /// Gwinches per second, for ranged weapons.
    pub projectile_speed: Option<f32>,
    /// The mastery attribute, for martial weapons. Caster weapons strike at
    /// three times the wielder's level instead.
    pub mastery: Option<Attribute>,
    /// Whether this is a spellcasting weapon (wand, staff), for AI rules.
    pub caster: bool,
    /// Its hits ignore armor: spirit attacks (Spirit).
    pub armor_ignoring: bool,
}

/// Where a unit is going.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MoveGoal {
    /// Walk to a point.
    Point(Vec2),
    /// Walk until within `range` of a unit, then do `then`.
    Approach {
        unit: UnitId,
        range: f32,
        then: PendingUse,
    },
    /// Walk directly away from a point.
    AwayFrom(Vec2),
}

/// What to do on arriving in range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingUse {
    Nothing,
    Skill { slot: u8 },
    Attack,
}

/// A creature on the field.
#[derive(Debug, Clone)]
pub struct Unit {
    pub id: UnitId,
    pub name: String,
    pub team: Team,
    pub kind: UnitKind,
    pub level: u8,
    pub professions: (Profession, Option<Profession>),
    /// The creature type as the wiki names it, for `CreatureType` filters.
    pub species: String,
    pub traits: Vec<CreatureTrait>,
    /// For summoned creatures, the unit that controls them.
    pub master: Option<UnitId>,
    /// The party slot this unit fills, for per-slot statistics.
    pub slot_index: Option<usize>,
    /// The foe roster index, for time-to-kill.
    pub foe_index: Option<usize>,
    /// Whether killing it grants experience (Air of Superiority).
    pub gives_experience: bool,
    /// Never moves (dummies, spirits, stationary foes).
    pub stationary: bool,
    /// Whether it regenerates naturally out of combat.
    pub natural_regeneration: bool,

    pub pos: Vec2,
    pub velocity: Vec2,
    pub facing: Vec2,
    pub radius: f32,
    pub goal: Option<MoveGoal>,

    /// Health in milli-HP.
    pub health: i32,
    /// Maximum health in whole HP, before Deep Wound and temporary effects.
    pub base_max_health: i32,
    /// Energy in 1/3000 units.
    pub energy: i32,
    /// Maximum energy in whole points, before overcast.
    pub base_max_energy: i32,
    /// Overcast, in 1/3000 energy units, recovering at one pip.
    pub overcast: i32,
    pub base_energy_pips: i32,
    pub base_health_pips: i32,

    pub bar: [Option<SlotState>; BAR],
    pub action: Action,
    /// Bumped whenever the current action changes, so stale events skip.
    pub action_generation: u32,
    /// The unit's current auto-attack target, kept across skills.
    pub attack_target: Option<UnitId>,
    /// When the next swing may begin.
    pub next_swing_at: SimTime,
    /// When the swing under way lands, if one is.
    pub swing_hits_at: Option<SimTime>,

    pub effects: Vec<ActiveEffect>,
    /// Effective attribute ranks from the build, indexed by `Attribute::index`.
    pub base_ranks: [u8; 42],
    pub armor: [PerDamageType; 5],
    pub weapon: Option<WeaponProfile>,
    /// Gear modifiers that hold all fight: rune energy, HM speed and so on.
    pub permanent: Vec<crate::stats::Modifier>,
    /// Gear effects evaluated as conditions change (insignias, chance mods).
    pub gear: Vec<crate::stats::GearEffect>,

    /// The last moment the unit was in active combat, for natural
    /// regeneration.
    pub last_combat: Option<SimTime>,
    /// When the controller may next decide.
    pub next_decision_at: SimTime,
    pub dead_at: Option<SimTime>,
    /// When Soul Reaping last paid out, for its three-per-15-seconds limit.
    pub soul_reaping: Vec<SimTime>,
    /// A fleshy corpse that nothing has exploited yet.
    pub corpse_available: bool,

    /// When it entered the fight: at the start, or when a skill created it.
    pub born_at: SimTime,
    /// For a spirit, the aura its ritual gives (T4.3.7).
    pub aura: Option<Aura>,
    /// For a created creature, its type (`spirits.ron` or `minions.ron`
    /// slug): only one allied spirit of each type may stand (ENG-32).
    pub creature_type: Option<String>,
    /// Death penalty, as a percentage off maximum health and energy
    /// (§10.11).
    pub death_penalty: u8,
    /// Morale boost, as a percentage onto maximum health and energy.
    pub morale: u8,

    /// For a foe, the group it aggroes with (AI-F1).
    pub group: Option<u16>,
    /// For a hero, its combat mode (AI-H2).
    pub hero_mode: HeroMode,
    /// The foe this unit is concentrating on: its last attack or offensive
    /// skill target. Heroes lock onto the player's (AI-H1).
    pub focus: Option<UnitId>,
    /// Where it stands when it has nothing to do: a hero's flag or formation
    /// point, a foe's spawn point.
    pub home: Vec2,
    /// Backs away from melee (AI-F5).
    pub kiter: bool,
}

/// A hero's combat mode (AI-H2, Hero).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeroMode {
    /// Attacks called targets, then targets engaging the party.
    #[default]
    Fight,
    /// Holds its position and fights only when engaged.
    Guard,
    /// Never attacks; uses only indirect skills.
    AvoidCombat,
}

/// A spirit's aura: the `SpiritAura` effect definitions of the skill that
/// created it, reaching units within the spirit's range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aura {
    /// The creating skill, as a fight skill index.
    pub skill: u16,
    /// The rank the creator had, which the aura's values use.
    pub rank: u8,
    /// Whether it reaches foes too (nature rituals) or only allies (binding
    /// rituals).
    pub affects_all: bool,
}

impl Unit {
    /// Whether the unit is alive.
    pub fn alive(&self) -> bool {
        !matches!(self.action, Action::Dead)
    }

    /// Current health in whole points, rounded down.
    pub fn health_points(&self) -> i32 {
        self.health.div_euclid(HEALTH_SCALE)
    }

    /// Current energy in whole points, rounded down. Can be negative when
    /// maximum energy fell below current (the wiki's energy hiding).
    pub fn energy_points(&self) -> i32 {
        self.energy.div_euclid(ENERGY_SCALE)
    }

    /// Whether the unit has a trait.
    pub fn has_trait(&self, t: CreatureTrait) -> bool {
        self.traits.contains(&t)
    }

    /// Whether the unit is activating a skill.
    pub fn activating(&self) -> Option<(u8, Target)> {
        match self.action {
            Action::Activating { slot, target, .. } => Some((slot, target)),
            _ => None,
        }
    }

    /// Whether the unit is moving this tick.
    pub fn moving(&self) -> bool {
        self.velocity.length_squared() > 0.0
    }

    /// Whether the unit is knocked down.
    pub fn knocked_down(&self) -> bool {
        matches!(self.action, Action::KnockedDown { .. })
    }

    /// The slot holding a skill, by fight skill index.
    pub fn slot_of(&self, skill: u16) -> Option<u8> {
        self.bar
            .iter()
            .position(|slot| slot.is_some_and(|slot| slot.skill == skill))
            .map(|index| index as u8)
    }

    /// Whether this unit counts as a caster for AI purposes.
    pub fn holds_caster_weapon(&self) -> bool {
        self.weapon.as_ref().is_some_and(|weapon| weapon.caster)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_pip_over_one_tick_is_a_whole_number_of_units() {
        // 2 HP per second per pip, over 50 ms.
        assert_eq!(2 * HEALTH_SCALE * crate::time::TICK_MS as i32 % 1000, 0);
        assert_eq!(2 * HEALTH_SCALE * crate::time::TICK_MS as i32 / 1000, 100);
        // One energy per three seconds per pip, over 50 ms.
        assert_eq!(ENERGY_SCALE * crate::time::TICK_MS as i32 % 3000, 0);
        assert_eq!(ENERGY_SCALE * crate::time::TICK_MS as i32 / 3000, 50);
    }
}
