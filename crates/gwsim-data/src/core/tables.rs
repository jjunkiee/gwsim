//! The shapes of the seven `data/core/*.ron` files.
//!
//! Each `*File` type is the whole of one file. Numbers live here; the
//! identities they hang off live in the enums beside this module.

use serde::{Deserialize, Serialize};

use super::{
    ArmorClassBonus, ArmorSlot, Attribute, Condition, InherentEffect, Profession, RangeBand,
    TitleTrack,
};
use crate::ids::AssumptionId;
use crate::provenance::Provenance;

/// A value the wiki does not publish.
///
/// Keeping "we do not know this yet" in the type stops a placeholder number
/// being mistaken for a researched one, which is the failure ENG-4 exists to
/// prevent.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Assumed<T> {
    /// A value taken from the wiki.
    Known(T),
    /// No value yet. The assumption register entry says who will set it.
    Pending(AssumptionId),
}

impl<T> Assumed<T> {
    /// The value, if there is one.
    pub fn known(&self) -> Option<&T> {
        match self {
            Assumed::Known(value) => Some(value),
            Assumed::Pending(_) => None,
        }
    }

    /// The assumption this value is waiting on, if it is waiting.
    pub fn pending_on(&self) -> Option<AssumptionId> {
        match self {
            Assumed::Known(_) => None,
            Assumed::Pending(id) => Some(*id),
        }
    }
}

// ---------------------------------------------------------------- professions

/// `data/core/professions.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfessionsFile {
    pub provenance: Provenance,
    /// Maximum energy every character has before armor adds any.
    pub base_energy: u16,
    /// Energy regeneration, in pips, before armor adds any.
    pub base_energy_regen_pips: u8,
    /// How many more regeneration pips a foe gets than a player.
    pub foe_extra_energy_regen_pips: u8,
    pub professions: Vec<ProfessionRecord>,
}

/// One profession's numbers.
///
/// Armor energy, regeneration and health are stored **per piece** rather than
/// as totals. The totals are derivable and the placement is not, and T1.4.6
/// needs the placement to work out which bonuses a partial armor set gives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfessionRecord {
    pub key: Profession,
    /// Armor rating of a full set of maximum-rating armor.
    pub base_armor: i16,
    /// Extra armor against a whole damage family, on every piece.
    #[serde(default)]
    pub armor_bonus: Option<ArmorClassBonus>,
    /// Maximum energy granted by armor, per piece.
    #[serde(default)]
    pub armor_energy: Vec<(ArmorSlot, u16)>,
    /// Energy regeneration pips granted by armor, per piece.
    #[serde(default)]
    pub armor_regen_pips: Vec<(ArmorSlot, u8)>,
    /// Maximum health granted by armor, per piece. Only the Dervish has any.
    #[serde(default)]
    pub armor_health: Vec<(ArmorSlot, u16)>,
    /// The energy a foe of this profession has.
    pub foe_energy: u16,
}

impl ProfessionRecord {
    /// Total maximum energy from a full armor set.
    pub fn armor_energy_total(&self) -> u16 {
        self.armor_energy.iter().map(|(_, amount)| amount).sum()
    }

    /// Total energy regeneration pips from a full armor set.
    pub fn armor_regen_pips_total(&self) -> u8 {
        self.armor_regen_pips.iter().map(|(_, amount)| amount).sum()
    }

    /// Total maximum health from a full armor set.
    pub fn armor_health_total(&self) -> u16 {
        self.armor_health.iter().map(|(_, amount)| amount).sum()
    }
}

// ---------------------------------------------------------------- attributes

/// `data/core/attributes.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributesFile {
    pub provenance: Provenance,
    pub attributes: Vec<AttributeRecord>,
}

/// One attribute's role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeRecord {
    pub key: Attribute,
    /// Whether this is its profession's primary attribute.
    ///
    /// Cross-checked at load against [`Attribute::is_primary`], which derives
    /// the same fact from the identity graph in code. A disagreement is a
    /// typo in one of the two, and the loader says which.
    pub primary: bool,
    /// The effects the attribute has whatever skills are equipped.
    ///
    /// A list rather than a single tag because Dagger Mastery has two: it
    /// scales dagger damage like every other weapon mastery *and* grants the
    /// double-strike chance. Recording only one of those would silently drop
    /// the other.
    #[serde(default)]
    pub inherent: Vec<InherentEffect>,
}

// ---------------------------------------------------------------- conditions

/// `data/core/conditions.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionsFile {
    pub provenance: Provenance,
    pub conditions: Vec<ConditionRecord>,
}

/// One condition's fixed effects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionRecord {
    pub key: Condition,
    /// Whether only fleshy creatures can suffer it.
    #[serde(default)]
    pub fleshy_only: bool,
    /// Whether spirits can suffer it. Burning is the only one.
    #[serde(default)]
    pub affects_spirits: bool,
    /// What it does. Conditions differ too much to share one set of fields.
    pub effects: Vec<ConditionEffect>,
}

/// Something a condition does.
///
/// A list of these rather than a wide struct of mostly-zero fields, because
/// the ten conditions have almost nothing in common: only four of them touch
/// health regeneration at all.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ConditionEffect {
    /// Health regeneration pips, negative for degeneration.
    HealthRegeneration(i8),
    /// Chance that a melee or missile attack misses, as a percentage.
    MissChance { percent: u8 },
    /// Change to movement speed, as a percentage.
    MovementSpeed { percent: i8 },
    /// Change to armor, with a floor below which it will not push the target.
    Armor { amount: i16, floor: Option<i16> },
    /// Change to maximum health, as a percentage, with a cap on the amount.
    MaxHealth { percent: i8, cap: i16 },
    /// Change to healing received, as a percentage.
    HealingReceived { percent: i8 },
    /// Change to every attribute rank. Ranks already at 0 are unaffected, and
    /// title-based ranks are never reduced.
    AllAttributes(i8),
    /// Change to the weapon's base damage, as a percentage. Bonus damage from
    /// attack skills is not affected.
    WeaponBaseDamage { percent: i8 },
    /// Multiplier on spell activation time, as a percentage of normal.
    SpellActivation { percent: u16 },
    /// Spells become interruptible by any damage.
    EasilyInterruptedSpells,
    /// Spreads to adjacent creatures of the same type.
    Contagious,
}

// -------------------------------------------------------------------- ranges

/// `data/core/ranges.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RangesFile {
    pub provenance: Provenance,
    /// Each named band and its distance in gwinches.
    pub bands: Vec<(RangeBand, f32)>,
}

// -------------------------------------------------------------------- levels

/// `data/core/levels.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelsFile {
    pub provenance: Provenance,
    /// Health at level 1. Characters and foes share one formula.
    pub health_at_level_1: u32,
    /// Health gained per level after the first.
    pub health_per_level: u32,
    /// In hard mode, extra health per level above [`Self::hm_bonus_from_level`].
    pub hm_bonus_health_per_level: u32,
    /// The level above which the hard-mode health bonus starts.
    pub hm_bonus_from_level: u8,
    /// A foe's core armor per level.
    pub foe_armor_per_level: i16,
    pub damage_multiplier: DamageMultiplier,
    pub hm_levels: HmLevelTables,
}

impl LevelsFile {
    /// Maximum health at a level, before runes, insignias and hard mode.
    pub fn health_at(&self, level: u8) -> u32 {
        let level = u32::from(level.max(1));
        self.health_at_level_1 + self.health_per_level * (level - 1)
    }

    /// The extra health a hard-mode foe gets for being above level 20.
    pub fn hm_bonus_health(&self, level: u8) -> u32 {
        let from = u32::from(self.hm_bonus_from_level);
        let level = u32::from(level);
        level
            .saturating_sub(from)
            .saturating_mul(self.hm_bonus_health_per_level)
    }

    /// A foe's core armor at a level, before profession bonuses.
    pub fn foe_armor_at(&self, level: u8) -> i16 {
        self.foe_armor_per_level * i16::from(level)
    }
}

/// The parameters of the skill-damage multiplier.
///
/// Stored as parameters rather than as a table so that levels between the
/// wiki's rows work, and so the formula is visible in the data (ENG-4).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageMultiplier {
    pub base: f64,
    pub level_coefficient: f64,
    pub offset: f64,
    pub divisor: f64,
}

impl DamageMultiplier {
    /// The multiplier armor-respecting skill damage takes at a level.
    ///
    /// Armor-ignoring damage always uses 1 instead, whatever the level.
    pub fn at(&self, level: u8) -> f64 {
        let exponent = (self.level_coefficient * f64::from(level) - self.offset) / self.divisor;
        self.base.powf(exponent)
    }
}

/// The hard-mode level mappings, one table per kind of creature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmLevelTables {
    pub non_boss: Vec<HmLevelRow>,
    pub boss: Vec<HmLevelRow>,
    pub ally: Vec<HmLevelRow>,
}

/// Which table to read a hard-mode level from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HmLevelKind {
    NonBoss,
    Boss,
    Ally,
}

/// One row of a hard-mode level mapping.
///
/// The wiki's rows **overlap** — normal-mode levels 3 and 4 appear in two
/// bands — so the table is an ordered list resolved first-match-wins, not a
/// map. A map would silently drop rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmLevelRow {
    /// Lowest normal-mode level this row covers.
    pub nm_min: u8,
    /// Highest normal-mode level this row covers.
    pub nm_max: u8,
    /// Lowest hard-mode level it maps to.
    pub hm_min: u8,
    /// Highest hard-mode level it maps to. Equal to `hm_min` on every row but
    /// the last non-boss one, which the wiki gives as a range.
    pub hm_max: u8,
}

impl HmLevelRow {
    /// Whether this row covers a normal-mode level.
    pub fn covers(&self, nm_level: u8) -> bool {
        (self.nm_min..=self.nm_max).contains(&nm_level)
    }

    /// The single hard-mode level this row gives, if it gives one.
    pub fn exact(&self) -> Option<u8> {
        (self.hm_min == self.hm_max).then_some(self.hm_min)
    }
}

impl HmLevelTables {
    /// The rows for a kind of creature.
    pub fn rows(&self, kind: HmLevelKind) -> &[HmLevelRow] {
        match kind {
            HmLevelKind::NonBoss => &self.non_boss,
            HmLevelKind::Boss => &self.boss,
            HmLevelKind::Ally => &self.ally,
        }
    }

    /// The first row covering a normal-mode level.
    pub fn row_for(&self, kind: HmLevelKind, nm_level: u8) -> Option<&HmLevelRow> {
        self.rows(kind).iter().find(|row| row.covers(nm_level))
    }

    /// The hard-mode level a normal-mode level maps to.
    ///
    /// Returns [`None`] when no row covers the level, and when the row that
    /// does gives a range rather than one level. Both cases mean the foe's own
    /// file has to state its hard-mode level; guessing here would invent data.
    pub fn hm_level(&self, kind: HmLevelKind, nm_level: u8) -> Option<u8> {
        self.row_for(kind, nm_level).and_then(HmLevelRow::exact)
    }
}

// --------------------------------------------------------------------- modes

/// `data/core/modes.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModesFile {
    pub provenance: Provenance,
    pub hard_mode: HardModeRules,
    pub reforged_mode: ReforgedModeRules,
    pub optional_modes: OptionalModeRules,
}

/// What hard mode changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HardModeRules {
    /// Extra movement speed, as a percentage.
    pub movement_speed_bonus_percent: u8,
    /// Extra attack speed, as a percentage.
    pub attack_speed_bonus_percent: u8,
    /// Foe skills taking longer than this to activate take half as long.
    pub activation_halved_above_seconds: f32,
    /// How much shorter foe recharges are, as a percentage.
    ///
    /// The wiki says only "shorter recharges" and gives no number, so this
    /// waits on A-032 rather than carrying an invented one.
    pub recharge_reduction_percent: Assumed<f32>,
    /// Added to a foe's normal-mode attribute ranks when its hard-mode ranks
    /// are unknown (A-005).
    pub missing_attribute_rank_bonus: u8,
    /// The cap that bonus is applied under.
    pub attribute_rank_cap: u8,
    pub hostile_spirit_armor: i16,
    pub hostile_spirit_energy: u16,
}

/// What Reforged Mode changes.
///
/// All of it is pre-Searing content, so none of it reaches M1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReforgedModeRules {
    /// Change to pre-Searing foe health, as a percentage.
    pub presearing_health_percent: i8,
    /// Change to pre-Searing foe armor, as a percentage.
    pub presearing_armor_percent: i8,
    /// Whether the mode changes any skill or attribute. It does not.
    pub changes_skills_or_attributes: bool,
}

/// The two opt-in modes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionalModeRules {
    pub dhuums_covenant: DhuumsCovenantRules,
    pub melandrus_accord: MelandrusAccordRules,
}

/// Dhuum's Covenant. It changes no combat rule; it only ends on death.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DhuumsCovenantRules {
    pub broken_by_death: bool,
}

/// Melandru's Accord. Every restriction narrows which builds are legal rather
/// than changing how combat works.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MelandrusAccordRules {
    pub skill_tomes_allowed: bool,
    /// Whether skill trainers offer more than their base pool.
    pub trainer_sells_unlocked_skills: bool,
    pub mercenary_heroes_allowed: bool,
    /// Whether account titles can be displayed and give passive effects.
    pub account_titles_available: bool,
}

// -------------------------------------------------------------------- titles

/// `data/core/titles.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TitlesFile {
    pub provenance: Provenance,
    pub tracks: Vec<TitleTrackRecord>,
}

/// One title track's rank-to-effective-rank table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TitleTrackRecord {
    pub key: TitleTrack,
    /// The highest title rank the track goes to.
    pub max_rank: u8,
    /// The effective attribute rank at each title rank, starting at rank 0.
    /// Indexed by title rank, so it holds `max_rank + 1` entries.
    pub effective_ranks: Vec<u8>,
}

impl TitleTrackRecord {
    /// The effective attribute rank at a title rank.
    ///
    /// Ranks above the track's maximum clamp to the last entry rather than
    /// failing: the tables plateau well before their maximum anyway.
    pub fn effective_rank(&self, title_rank: u8) -> Option<u8> {
        if self.effective_ranks.is_empty() {
            return None;
        }
        let index = usize::from(title_rank).min(self.effective_ranks.len() - 1);
        self.effective_ranks.get(index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels() -> LevelsFile {
        LevelsFile {
            provenance: Provenance {
                sources: vec!["https://wiki.guildwars.com/wiki/Level".to_owned()],
                crawled: "2026-09-22".parse().unwrap(),
                review: crate::provenance::ReviewStatus::Draft,
                reviewed_by: None,
                assumptions: Vec::new(),
                notes: String::new(),
            },
            health_at_level_1: 100,
            health_per_level: 20,
            hm_bonus_health_per_level: 20,
            hm_bonus_from_level: 20,
            foe_armor_per_level: 3,
            damage_multiplier: DamageMultiplier {
                base: 2.0,
                level_coefficient: 3.0,
                offset: 60.0,
                divisor: 40.0,
            },
            hm_levels: HmLevelTables {
                non_boss: vec![
                    HmLevelRow {
                        nm_min: 0,
                        nm_max: 0,
                        hm_min: 20,
                        hm_max: 20,
                    },
                    HmLevelRow {
                        nm_min: 0,
                        nm_max: 4,
                        hm_min: 22,
                        hm_max: 22,
                    },
                    HmLevelRow {
                        nm_min: 3,
                        nm_max: 9,
                        hm_min: 23,
                        hm_max: 23,
                    },
                    HmLevelRow {
                        nm_min: 20,
                        nm_max: 24,
                        hm_min: 26,
                        hm_max: 26,
                    },
                    HmLevelRow {
                        nm_min: 34,
                        nm_max: 34,
                        hm_min: 36,
                        hm_max: 40,
                    },
                ],
                boss: vec![HmLevelRow {
                    nm_min: 19,
                    nm_max: 22,
                    hm_min: 29,
                    hm_max: 29,
                }],
                ally: vec![HmLevelRow {
                    nm_min: 0,
                    nm_max: 20,
                    hm_min: 20,
                    hm_max: 20,
                }],
            },
        }
    }

    #[test]
    fn health_follows_the_wiki_table() {
        let levels = levels();
        assert_eq!(levels.health_at(1), 100);
        assert_eq!(levels.health_at(5), 180);
        assert_eq!(levels.health_at(10), 280);
        assert_eq!(levels.health_at(20), 480);
        assert_eq!(levels.health_at(26), 600);
        assert_eq!(levels.health_at(42), 920);
    }

    #[test]
    fn hard_mode_bonus_health_starts_above_twenty() {
        let levels = levels();
        assert_eq!(levels.hm_bonus_health(20), 0);
        assert_eq!(levels.hm_bonus_health(22), 40);
        assert_eq!(levels.hm_bonus_health(26), 120);
        // A level-26 hard-mode foe has 600 + 120 = 720 health (DESIGN 20.2).
        assert_eq!(levels.health_at(26) + levels.hm_bonus_health(26), 720);
    }

    #[test]
    fn foe_armor_is_three_per_level() {
        let levels = levels();
        assert_eq!(levels.foe_armor_at(1), 3);
        assert_eq!(levels.foe_armor_at(20), 60);
        assert_eq!(levels.foe_armor_at(26), 78);
    }

    #[test]
    fn the_damage_multiplier_matches_the_wiki_table() {
        let multiplier = levels().damage_multiplier;
        let close = |got: f64, want: f64| {
            assert!((got - want).abs() < 0.001, "expected {want}, got {got}");
        };
        close(multiplier.at(1), 0.372);
        close(multiplier.at(5), 0.459);
        close(multiplier.at(10), 0.595);
        close(multiplier.at(15), 0.771);
        close(multiplier.at(20), 1.000);
        close(multiplier.at(26), 1.366);
        close(multiplier.at(42), 3.138);
    }

    #[test]
    fn a_level_twenty_foe_becomes_twenty_six_in_hard_mode() {
        let tables = levels().hm_levels;
        assert_eq!(tables.hm_level(HmLevelKind::NonBoss, 20), Some(26));
    }

    #[test]
    fn overlapping_rows_resolve_to_the_first_match() {
        // Levels 3 and 4 sit in both the 0-4 and the 3-9 band. The wiki lists
        // 0-4 first, so that is the answer; a map keyed by level could not
        // even hold both rows.
        let tables = levels().hm_levels;
        assert_eq!(tables.hm_level(HmLevelKind::NonBoss, 0), Some(20));
        assert_eq!(tables.hm_level(HmLevelKind::NonBoss, 4), Some(22));
        assert_eq!(tables.hm_level(HmLevelKind::NonBoss, 3), Some(22));
        assert_eq!(tables.hm_level(HmLevelKind::NonBoss, 9), Some(23));
    }

    #[test]
    fn a_range_valued_row_gives_no_single_level() {
        // The wiki maps normal level 34 to "36-40". There is no right answer,
        // so the foe's own file has to say.
        let tables = levels().hm_levels;
        assert_eq!(tables.hm_level(HmLevelKind::NonBoss, 34), None);
        assert!(tables.row_for(HmLevelKind::NonBoss, 34).is_some());
    }

    #[test]
    fn an_uncovered_level_gives_nothing() {
        let tables = levels().hm_levels;
        assert_eq!(tables.hm_level(HmLevelKind::NonBoss, 32), None);
        assert!(tables.row_for(HmLevelKind::NonBoss, 32).is_none());
    }

    #[test]
    fn bosses_and_allies_use_their_own_tables() {
        let tables = levels().hm_levels;
        assert_eq!(tables.hm_level(HmLevelKind::Boss, 20), Some(29));
        assert_eq!(tables.hm_level(HmLevelKind::Ally, 20), Some(20));
        assert_eq!(tables.hm_level(HmLevelKind::NonBoss, 20), Some(26));
    }

    #[test]
    fn title_ranks_plateau() {
        let track = TitleTrackRecord {
            key: TitleTrack::Asura,
            max_rank: 10,
            effective_ranks: vec![0, 3, 6, 9, 12, 15, 15, 15, 15, 15, 15],
        };
        assert_eq!(track.effective_rank(0), Some(0));
        assert_eq!(track.effective_rank(4), Some(12));
        assert_eq!(track.effective_rank(5), Some(15));
        assert_eq!(track.effective_rank(10), Some(15));
        // Above the track's maximum, clamp rather than fail.
        assert_eq!(track.effective_rank(12), Some(15));
    }

    #[test]
    fn a_pending_value_yields_no_number() {
        let pending: Assumed<f32> = Assumed::Pending("A-032".parse().unwrap());
        assert_eq!(pending.known(), None);
        assert_eq!(
            pending.pending_on().map(|id| id.to_string()),
            Some("A-032".to_owned())
        );

        let known = Assumed::Known(25.0_f32);
        assert_eq!(known.known(), Some(&25.0));
        assert_eq!(known.pending_on(), None);
    }
}
