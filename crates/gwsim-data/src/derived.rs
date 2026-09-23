//! Derived stats: what a build and a level turn into before any fight starts.
//!
//! Everything here is a pure function of the build, the data and the level.
//! Nothing that depends on what is happening in a fight belongs here — an
//! insignia that grants armor "while enchanted" is an engine effect (WP4.3),
//! and this module reports it as a condition rather than adding it in.

use std::collections::BTreeMap;

use crate::build::Build;
use crate::core::{
    ArmorSlot, Attribute, CoreData, DamageType, HmLevelKind, InherentEffect, Profession,
};
use crate::dataset::DataSet;
use crate::foe::Foe;
use crate::items::{Rune, RuneBonus, RuneKind};

/// The highest rank attribute points alone can buy.
pub const MAX_POINTS_RANK: u8 = 12;
/// The attribute-point budget of a level-20 character with both quests done.
pub const MAX_POINTS: u16 = 200;
/// The cap on an effective attribute rank.
///
/// A weapon's "+1 (20% chance)" mod can push one rank to 21 for a moment.
/// That is a per-activation roll, so the engine applies it, not this module.
pub const MAX_EFFECTIVE_RANK: u8 = 20;

/// The running cost of each attribute rank, indexed by rank.
///
/// A table rather than a formula: the per-rank cost steps from +1 to +2 at
/// rank 8 and to +3 at rank 11, so nothing closed-form fits it.
const CUMULATIVE_COST: [u16; 13] = [0, 1, 3, 6, 10, 15, 21, 28, 37, 48, 61, 77, 97];

/// The points needed to reach a rank.
///
/// Ranks above 12 cost what rank 12 costs, since points cannot buy them.
pub fn attribute_cost(rank: u8) -> u16 {
    let index = usize::from(rank.min(MAX_POINTS_RANK));
    CUMULATIVE_COST[index]
}

/// The highest rank a budget can buy.
pub fn rank_for_points(budget: u16) -> u8 {
    CUMULATIVE_COST
        .iter()
        .rposition(|cost| *cost <= budget)
        .unwrap_or(0) as u8
}

/// The points a build spends.
pub fn points_spent(build: &Build) -> u16 {
    build
        .attribute_points
        .values()
        .map(|rank| attribute_cost(*rank))
        .sum()
}

/// The rank each attribute ends up at, once runes and headgear are counted.
///
/// Three rules decide this, and all three are easy to get wrong:
///
/// - only the **highest** rune for an attribute counts;
/// - runes fit only the **primary** profession's armor;
/// - headgear adds +1 to one primary-profession attribute, and **does** stack
///   with a rune.
pub fn effective_ranks(build: &Build, data: &DataSet) -> BTreeMap<Attribute, u8> {
    let mut ranks: BTreeMap<Attribute, u8> = build.attribute_points.clone();

    // The best rune per attribute, not the sum of them.
    let mut best_rune: BTreeMap<Attribute, u8> = BTreeMap::new();
    for rune in worn_runes(build, data) {
        if let RuneKind::Attribute { attribute, .. } = &rune.kind
            && build.can_rune(*attribute)
            && let Some(RuneBonus::AttributeRank(bonus)) = rune.bonus
        {
            let entry = best_rune.entry(*attribute).or_insert(0);
            *entry = (*entry).max(bonus);
        }
    }
    for (attribute, bonus) in best_rune {
        *ranks.entry(attribute).or_insert(0) += bonus;
    }

    if let Some(attribute) = build.headgear_attribute
        && attribute.profession() == build.primary
    {
        *ranks.entry(attribute).or_insert(0) += 1;
    }

    for rank in ranks.values_mut() {
        *rank = (*rank).min(MAX_EFFECTIVE_RANK);
    }
    ranks
}

/// Every rune fitted to a build's armor, in slot order.
pub fn worn_runes<'a>(build: &'a Build, data: &'a DataSet) -> impl Iterator<Item = &'a Rune> {
    build.armor.iter().filter_map(move |piece| {
        let slug = piece.rune.as_ref()?;
        data.runes
            .as_ref()?
            .value
            .runes
            .iter()
            .find(|rune| rune.slug == *slug)
    })
}

/// Maximum health.
///
/// The subtle part is that **every** attribute rune's health penalty applies,
/// even when its bonus is suppressed by a better rune for the same attribute,
/// while only the **best** Vigor rune's bonus counts. Vitae stacks with
/// everything, including itself.
pub fn max_health(build: &Build, level: u8, data: &DataSet, core: &CoreData) -> i32 {
    let mut health = core.levels.health_at(level) as i32;

    let mut best_vigor = 0i32;
    for rune in worn_runes(build, data) {
        // Penalties apply whatever else the rune does.
        health -= i32::from(rune.health_penalty);

        match (&rune.kind, rune.bonus) {
            (RuneKind::Vigor(_), Some(RuneBonus::MaxHealth(bonus))) => {
                best_vigor = best_vigor.max(i32::from(bonus));
            }
            (RuneKind::Vitae, Some(RuneBonus::MaxHealth(bonus))) => {
                health += i32::from(bonus);
            }
            _ => {}
        }
    }
    health += best_vigor;

    // Only the Dervish has armor that grants health, and only on the chest.
    health += i32::from(core.profession(build.primary).armor_health_total());

    // Max health can never fall below 1.
    health.max(1)
}

/// Maximum energy and regeneration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnergyStats {
    pub max: i32,
    /// Pips. Each restores 1 energy every 3 seconds.
    pub regen_pips: u8,
}

/// Maximum energy and regeneration, before anything temporary.
pub fn energy(build: &Build, data: &DataSet, core: &CoreData) -> EnergyStats {
    let mut max = i32::from(core.max_energy(build.primary));

    // Energy Storage is an attribute inherent effect, so it is looked up
    // through the data rather than hard-coded against the Elementalist.
    let ranks = effective_ranks(build, data);
    for (attribute, rank) in &ranks {
        let record = core.attribute(*attribute);
        if record.inherent.contains(&InherentEffect::EnergyStorage)
            && attribute.profession() == build.primary
        {
            max += i32::from(*rank) * 3;
        }
    }

    // Attunement runes stack, unlike most runes.
    for rune in worn_runes(build, data) {
        if rune.kind == RuneKind::Attunement
            && let Some(RuneBonus::MaxEnergy(bonus)) = rune.bonus
        {
            max += i32::from(bonus);
        }
    }

    max += weapon_energy(build, data);

    EnergyStats {
        max: max.max(0),
        regen_pips: core.energy_regen_pips(build.primary),
    }
}

/// Energy from the weapon and off-hand.
fn weapon_energy(build: &Build, data: &DataSet) -> i32 {
    let Some(weapons) = data.weapons.as_ref() else {
        return 0;
    };
    let lookup = |slug: &Option<crate::ids::Slug>| -> i32 {
        let Some(slug) = slug else { return 0 };
        weapons
            .value
            .weapons
            .iter()
            .find(|weapon| weapon.slug == *slug)
            .and_then(|weapon| weapon.energy)
            .map(i32::from)
            .unwrap_or(0)
    };
    lookup(&build.weapon_set.main) + lookup(&build.weapon_set.offhand)
}

/// Armor per piece, per damage type, before anything temporary.
#[derive(Debug, Clone, PartialEq)]
pub struct ArmorProfile {
    /// Armor per piece, in [`ArmorSlot::ALL`] order.
    pub pieces: [PerDamageType; 5],
    /// Bonuses that only apply while something is true.
    ///
    /// Kept separate on purpose: folding a conditional insignia into resting
    /// armor would overstate every M1 caster by up to 15 (T1.4.1 §4).
    pub conditional: Vec<ConditionalArmor>,
}

/// An armor figure per damage type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerDamageType {
    /// Armor against a damage type with no specific bonus.
    pub base: i16,
    /// Armor against each damage type, in [`DamageType::ALL`] order.
    pub by_type: [i16; 11],
}

impl PerDamageType {
    /// Armor against one damage type.
    pub fn against(&self, damage_type: DamageType) -> i16 {
        self.by_type[damage_type.index()]
    }
}

/// A bonus that applies only while a condition holds.
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionalArmor {
    pub piece: ArmorSlot,
    /// What has to be true, in the insignia's own words.
    pub condition: String,
    pub amount: i16,
}

/// Armor per piece and damage type.
///
/// This produces the *inputs* to the armor calculation; WP3.5 does the
/// calculation itself.
pub fn armor_profile(build: &Build, core: &CoreData) -> ArmorProfile {
    let record = core.profession(build.primary);
    let base = record.base_armor;

    let mut by_type = [base; 11];
    if let Some(bonus) = record.armor_bonus {
        for damage_type in DamageType::ALL {
            if bonus.against.covers(damage_type) {
                by_type[damage_type.index()] = base + bonus.amount;
            }
        }
    }

    let piece = PerDamageType { base, by_type };

    ArmorProfile {
        pieces: [piece; 5],
        // Insignia effects are DSL values until WP1.5 gives them a shape, so
        // nothing can be read out of them yet. The field exists so callers are
        // written against the right model from the start.
        conditional: Vec::new(),
    }
}

// ------------------------------------------------------------ skill scaling

/// A skill value at a rank, under the wiki's `{{gr}}` rule.
///
/// `round(at0 + rank × (at15 − at0) / 15)`. It extrapolates past rank 15, up
/// to the cap of 20.
pub fn scaled(at0: i32, at15: i32, rank: u8) -> i32 {
    let span = f64::from(at15 - at0);
    let value = f64::from(at0) + f64::from(rank) * span / 15.0;
    value.round() as i32
}

/// A title-scaled value, under `{{gr2}}`.
///
/// The title's rank is first turned into an effective attribute rank through
/// its table (A-020), and the usual scaling applies from there.
pub fn title_scaled(
    r0: i32,
    rmax: i32,
    title_rank: u8,
    core: &CoreData,
    track: crate::core::TitleTrack,
) -> Option<i32> {
    let effective = core.title_track(track)?.effective_rank(title_rank)?;
    Some(scaled(r0, rmax, effective))
}

/// Whether a published triplet is self-consistent.
///
/// The wiki gives values at ranks 0, 12 and 15; the middle one is derivable,
/// so a mismatch means a transcription error somewhere.
pub fn check_triplet(r0: i32, r12: i32, r15: i32) -> bool {
    scaled(r0, r15, 12) == r12
}

// -------------------------------------------------------------- foe stats

/// A foe's level in a mode.
///
/// Per-foe data wins. Falling back to the mapping in `levels.ron` is
/// deliberately **not** done here: that table has overlaps and gaps, and a
/// guess would be indistinguishable from a real value.
pub fn foe_level(foe: &Foe, hard_mode: bool) -> Option<u8> {
    if !hard_mode {
        return Some(foe.level.nm);
    }
    foe.level.hm
}

/// The level a normal-mode level maps to in hard mode, where the table says.
pub fn mapped_hm_level(core: &CoreData, nm_level: u8, kind: HmLevelKind) -> Option<u8> {
    core.levels.hm_levels.hm_level(kind, nm_level)
}

/// A foe's maximum health.
pub fn foe_max_health(core: &CoreData, level: u8, hard_mode: bool) -> u32 {
    let base = core.levels.health_at(level);
    if hard_mode {
        base + core.levels.hm_bonus_health(level)
    } else {
        base
    }
}

/// A foe's energy, and how fast it regenerates.
pub fn foe_energy(core: &CoreData, profession: Profession) -> EnergyStats {
    EnergyStats {
        max: i32::from(core.profession(profession).foe_energy),
        regen_pips: core.foe_energy_regen_pips(profession),
    }
}

/// A foe's armor against a damage type.
///
/// The foe's own wiki table wins. Otherwise it is `3 × level` plus whatever
/// damage-type bonus its profession carries.
///
/// **The hard-mode rule is not "more armor".** Foes below level 20 in normal
/// mode get level-20 armor in hard mode; foes above 20 keep their normal-mode
/// armor. Higher hard-mode levels do *not* raise armor.
pub fn foe_armor(foe: &Foe, core: &CoreData, damage_type: DamageType, hard_mode: bool) -> i16 {
    foe_armor_with(foe, &foe.armor, core, damage_type, hard_mode)
}

/// A foe's armor using a given table, such as a variant's own (F2.11).
///
/// A table's figures hold in both modes: hard-mode foes gain no armor for
/// their higher level (Hard mode). Without a table, the level formula and the
/// profession's bonus apply.
pub fn foe_armor_with(
    foe: &Foe,
    table: &crate::foe::ArmorTable,
    core: &CoreData,
    damage_type: DamageType,
    hard_mode: bool,
) -> i16 {
    if let Some(armor) = table.against(damage_type) {
        return armor;
    }

    let nm_level = foe.level.nm;
    let level = if hard_mode {
        nm_level.max(20)
    } else {
        nm_level
    };

    let mut armor = core.levels.foe_armor_at(level);
    let record = core.profession(foe.professions.0);
    if let Some(bonus) = record.armor_bonus
        && bonus.against.covers(damage_type)
    {
        armor += bonus.amount;
    }
    armor
}

/// A foe's attribute ranks in a mode.
///
/// Returns the ranks and whether A-005 had to supply them, so that a caller
/// can record the assumption as used.
pub fn foe_attributes(foe: &Foe, core: &CoreData, hard_mode: bool) -> (Vec<(Attribute, u8)>, bool) {
    if !hard_mode {
        return (foe.attributes.nm.clone(), false);
    }
    if let Some(ranks) = &foe.attributes.hm {
        return (ranks.clone(), false);
    }

    // A-005: normal-mode ranks plus a bonus, capped.
    let rules = &core.modes.hard_mode;
    let ranks = foe
        .attributes
        .nm
        .iter()
        .map(|(attribute, rank)| {
            let raised = rank.saturating_add(rules.missing_attribute_rank_bonus);
            (*attribute, raised.min(rules.attribute_rank_cap))
        })
        .collect();
    (ranks, true)
}
