//! The shape of a `data/creatures/foes/**/*.ron` file, and the files that
//! describe creatures a skill creates (§7.1, §9.3).

use serde::{Deserialize, Serialize};

use crate::core::{Attribute, Campaign, DamageType, Profession};
use crate::ids::{SkillId, Slug, WikiTitle};
use crate::provenance::Provenance;
use crate::units::Seconds;

/// A value that differs between normal and hard mode.
///
/// `hm` is optional because the wiki gives hard-mode figures for very few
/// foes. What a missing value *means* depends on the field, and the rules are
/// not the same:
///
/// - a **level** must have both, and T1.2.8 rejects a foe missing its hard-mode
///   level — the level mapping in `levels.ron` is a fallback the foe file
///   should have consulted, not something to guess at load;
/// - **attribute ranks** may omit it, and A-005 then supplies normal-mode
///   ranks plus five, capped at twenty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeValue<T> {
    /// The normal-mode value.
    pub nm: T,
    /// The hard-mode value, where the wiki gives one.
    #[serde(default = "none")]
    pub hm: Option<T>,
}

fn none<T>() -> Option<T> {
    None
}

impl<T> ModeValue<T> {
    /// The value for a mode, falling back to normal mode.
    pub fn get(&self, hard_mode: bool) -> &T {
        match (hard_mode, &self.hm) {
            (true, Some(value)) => value,
            _ => &self.nm,
        }
    }

    /// Whether a hard-mode value was recorded.
    pub fn has_hm(&self) -> bool {
        self.hm.is_some()
    }
}

/// One foe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Foe {
    pub name: String,
    pub wiki: WikiTitle,
    /// The group it belongs to, which is also the folder it lives in.
    pub affiliation: String,
    /// Its creature type, as the wiki names it.
    pub species: String,
    /// What it is made of, which decides which conditions can touch it.
    #[serde(default)]
    pub traits: Vec<CreatureTrait>,

    /// Its professions. The second is [`None`] for most foes.
    pub professions: (Profession, Option<Profession>),

    pub level: ModeValue<u8>,
    /// Attribute ranks. A missing hard-mode list falls back to A-005.
    pub attributes: ModeValue<Vec<(Attribute, u8)>>,

    #[serde(default)]
    pub skills: Vec<FoeSkill>,
    /// Alternative loadouts the wiki lists for the same foe, such as the
    /// Kournan Guard's axe and hammer forms (A-007).
    #[serde(default)]
    pub variants: Vec<FoeVariant>,

    #[serde(default)]
    pub armor: ArmorTable,
    /// Health that overrides the level formula. Bosses and spirits have one.
    #[serde(default)]
    pub health_override: Option<u32>,
    /// Energy that overrides the profession default.
    #[serde(default)]
    pub energy: Option<u32>,
    #[serde(default)]
    pub weapon: Option<FoeWeapon>,

    #[serde(default)]
    pub boss: bool,
    #[serde(default)]
    pub ai_tags: Vec<AiTag>,

    pub provenance: Provenance,
}

/// What a creature is made of.
///
/// This decides which conditions and skills can affect it, so it is a list of
/// facts rather than a single species name. Extended as foes need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CreatureTrait {
    /// Can suffer Bleeding, Disease and Poison, and leaves an exploitable
    /// corpse.
    Fleshy,
    /// Immune to hexes and to every condition but Burning.
    Spirit,
    Undead,
    Construct,
    Elemental,
    Plant,
    Animal,
    Demon,
    /// Created by a skill rather than spawned by the area.
    Summoned,
    Minion,
}

/// A skill on a foe's bar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoeSkill {
    pub skill: SkillRef,
    /// Whether the foe only carries it in hard mode. Non-boss foes gain an
    /// elite skill there.
    #[serde(default)]
    pub hm_only: bool,
}

/// A reference to a skill, by either of the two ways a skill is named.
///
/// Both spellings exist because a foe's wiki page names skills in words while
/// a template code carries numbers. T1.2.8 resolves either to a real skill and
/// reports the ones that do not resolve.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SkillRef {
    /// By file name, such as `energy-surge`.
    Slug(Slug),
    /// By template id.
    Id(SkillId),
}

/// An alternative loadout for the same foe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoeVariant {
    /// What tells this variant from the others, such as `axe`.
    pub name: String,
    #[serde(default)]
    pub weapon: Option<FoeWeapon>,
    /// A replacement bar. [`None`] keeps the foe's own.
    #[serde(default)]
    pub skills: Option<Vec<FoeSkill>>,
}

/// A foe's armor, as the wiki's armor table gives it.
///
/// The wiki publishes these per damage type, and often at a level that is not
/// stated on the same table, which is why `level_context` and `note` exist:
/// an armor figure without the level it was measured at cannot be scaled.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmorTable {
    /// Armor against any damage type not listed below.
    #[serde(default)]
    pub default: Option<i16>,
    /// Armor against particular damage types.
    #[serde(default)]
    pub per_type: Vec<(DamageType, i16)>,
    /// The level these figures were read at, where the wiki says.
    #[serde(default)]
    pub level_context: Option<u8>,
    /// Anything a later reader needs to know about the figures.
    #[serde(default)]
    pub note: String,
}

impl ArmorTable {
    /// Armor against a damage type, if this table says.
    pub fn against(&self, damage_type: DamageType) -> Option<i16> {
        self.per_type
            .iter()
            .find(|(kind, _)| *kind == damage_type)
            .map(|(_, armor)| *armor)
            .or(self.default)
    }

    /// Whether the table says anything at all.
    pub fn is_empty(&self) -> bool {
        self.default.is_none() && self.per_type.is_empty()
    }
}

/// What a foe attacks with.
///
/// Every field is optional because creature pages do not record weapons, which
/// is A-004: the values are inferred from the player weapon tables at the
/// foe's level, and a file that leaves them out is saying so honestly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoeWeapon {
    /// A weapon type from `items/weapons.ron`.
    pub weapon_type: Slug,
    /// Minimum and maximum damage.
    #[serde(default)]
    pub damage: Option<(u16, u16)>,
    #[serde(default)]
    pub attack_interval: Option<Seconds>,
}

/// How a foe behaves, beyond what its skills imply.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AiTag {
    /// Backs away from melee.
    Kiter,
    /// Never moves.
    Stationary,
    /// Stays with its group rather than chasing.
    Anchored,
    /// Runs for help when hurt.
    Fleer,
    /// Behaviour that needs code, named here.
    Script(String),
}

// ------------------------------------------------------------- created creatures

/// `data/creatures/heroes.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeroesFile {
    pub provenance: Provenance,
    pub heroes: Vec<Hero>,
}

/// A hero.
///
/// D17: a hero's identity matters only for its profession and whether the
/// account has it, so nothing else is recorded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hero {
    pub slug: Slug,
    pub name: String,
    /// Fixed, and the one thing about a hero that changes a build.
    pub profession: Profession,
    pub availability: HeroAvailability,
    /// Whether the hero takes a profession chosen by the player.
    #[serde(default)]
    pub mercenary: bool,
}

/// How a hero is obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeroAvailability {
    /// Comes with a campaign.
    Campaign(Campaign),
    /// Added by Guild Wars Reforged.
    Reforged,
    /// Bought with a mercenary slot.
    Mercenary,
}

/// `data/creatures/minions.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinionsFile {
    pub provenance: Provenance,
    pub minions: Vec<Minion>,
}

/// A minion an Animate skill creates.
///
/// The rules are expressed as scaling parameters rather than flat numbers
/// because every one of them moves with Death Magic rank. Filled in by WP4.3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Minion {
    pub slug: Slug,
    pub name: String,
    /// Level at Death Magic 0 and 15.
    pub level: (u8, u8),
    #[serde(default)]
    pub armor: Option<i16>,
    #[serde(default)]
    pub weapon: Option<FoeWeapon>,
    /// Health degeneration pips once it starts to decay.
    #[serde(default)]
    pub degeneration: Option<i8>,
    #[serde(default)]
    pub traits: Vec<CreatureTrait>,
}

/// `data/creatures/spirits.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpiritsFile {
    pub provenance: Provenance,
    pub spirits: Vec<Spirit>,
}

/// A spirit a ritual creates.
///
/// Health and armor are A-015: the wiki's figures are unofficial, and friendly
/// spirit armor is a default rather than a measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spirit {
    pub slug: Slug,
    pub name: String,
    /// Health at rank 0 and 15 of the creating attribute.
    #[serde(default)]
    pub health: Option<(u32, u32)>,
    #[serde(default)]
    pub armor: Option<i16>,
    /// How far its aura reaches, in gwinches.
    #[serde(default)]
    pub range: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Kournan Seer as DESIGN §20.2 describes it. T1.2.4's done criterion.
    const SEER: &str = r#"(
    name: "Kournan Seer",
    wiki: "Kournan Seer",
    affiliation: "kournan",
    species: "Human",
    traits: [Fleshy],
    professions: (Mesmer, None),
    level: (nm: 20, hm: Some(26)),
    attributes: (
        nm: [(DominationMagic, 15), (InspirationMagic, 14)],
        hm: Some([(DominationMagic, 20), (InspirationMagic, 14)]),
    ),
    skills: [
        (skill: Slug("drain-enchantment")),
        (skill: Slug("enchanters-conundrum")),
        (skill: Slug("power-spike")),
        (skill: Slug("shatter-enchantment")),
    ],
    armor: (default: Some(60), level_context: Some(20)),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Kournan_Seer"],
        crawled: "2026-09-22",
        review: NumbersOnly,
        assumptions: ["A-004"],
    ),
)"#;

    fn seer() -> Foe {
        ron::from_str(SEER).expect("the Kournan Seer should parse")
    }

    #[test]
    fn the_kournan_seer_parses() {
        let foe = seer();
        assert_eq!(foe.name, "Kournan Seer");
        assert_eq!(foe.professions, (Profession::Mesmer, None));
        assert_eq!(foe.level.nm, 20);
        assert_eq!(foe.level.hm, Some(26));
        assert_eq!(foe.skills.len(), 4);
        assert!(!foe.boss);
    }

    #[test]
    fn the_kournan_seer_round_trips() {
        let original = seer();
        let text = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("should serialise");
        let back: Foe = ron::from_str(&text).expect("should parse again");
        assert_eq!(back, original);
    }

    #[test]
    fn mode_values_fall_back_to_normal_mode() {
        let foe = seer();
        assert_eq!(*foe.level.get(false), 20);
        assert_eq!(*foe.level.get(true), 26);

        // Domination rises to 20 in hard mode; Inspiration stays at 14.
        assert_eq!(foe.attributes.get(true)[0].1, 20);
        assert_eq!(foe.attributes.get(false)[0].1, 15);
    }

    #[test]
    fn a_foe_without_hard_mode_attributes_says_so() {
        // The Kournan Zealot's ranks are not on the wiki at all, so A-005
        // supplies them. The file must be able to express "we do not know"
        // rather than inventing a number.
        let text = SEER.replace(
            "hm: Some([(DominationMagic, 20), (InspirationMagic, 14)]),",
            "hm: None,",
        );
        let foe: Foe = ron::from_str(&text).expect("should parse");
        assert!(!foe.attributes.has_hm());
        // And falling back gives the normal-mode ranks, not an empty list.
        assert_eq!(foe.attributes.get(true).len(), 2);
        assert_eq!(foe.attributes.get(true)[0].1, 15);
    }

    #[test]
    fn armor_falls_back_to_the_default() {
        let foe = seer();
        assert_eq!(foe.armor.against(DamageType::Fire), Some(60));
        assert_eq!(foe.armor.against(DamageType::Slashing), Some(60));
        assert!(!foe.armor.is_empty());
    }

    #[test]
    fn per_type_armor_beats_the_default() {
        // The Kournan Guard's wiki table gives two figures, 116 and 96.
        let table = ArmorTable {
            default: Some(96),
            per_type: vec![
                (DamageType::Slashing, 116),
                (DamageType::Piercing, 116),
                (DamageType::Blunt, 116),
            ],
            level_context: Some(20),
            note: String::new(),
        };
        assert_eq!(table.against(DamageType::Slashing), Some(116));
        assert_eq!(table.against(DamageType::Fire), Some(96));
    }

    #[test]
    fn an_empty_armor_table_admits_it() {
        let table = ArmorTable::default();
        assert!(table.is_empty());
        assert_eq!(table.against(DamageType::Fire), None);
    }

    #[test]
    fn skills_can_be_named_either_way() {
        #[derive(Deserialize)]
        struct Holder {
            skill: SkillRef,
        }
        let by_slug: Holder = ron::from_str(r#"(skill: Slug("energy-surge"))"#).unwrap();
        assert_eq!(
            by_slug.skill,
            SkillRef::Slug("energy-surge".parse().unwrap())
        );

        let by_id: Holder = ron::from_str("(skill: Id(1234))").unwrap();
        assert_eq!(by_id.skill, SkillRef::Id(SkillId(1234)));
    }

    #[test]
    fn an_invalid_slug_in_a_skill_reference_is_rejected() {
        // The Slug newtype does the checking, so a reference cannot name a
        // file that could not exist.
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct Holder {
            skill: SkillRef,
        }
        assert!(ron::from_str::<Holder>(r#"(skill: Slug("Energy Surge"))"#).is_err());
    }

    #[test]
    fn a_variant_can_replace_the_weapon_alone() {
        // A-007: the Kournan Guard has axe and hammer variants that differ
        // only in what they hold.
        let text = r#"(
            name: "axe",
            weapon: Some((weapon_type: "axe", damage: Some((6, 28)))),
            skills: None,
        )"#;
        let variant: FoeVariant = ron::from_str(text).expect("should parse");
        assert_eq!(variant.name, "axe");
        assert!(variant.skills.is_none());
        assert_eq!(variant.weapon.unwrap().damage, Some((6, 28)));
    }

    #[test]
    fn a_hard_mode_only_skill_is_marked() {
        // Non-boss foes gain an elite skill in hard mode.
        let skill: FoeSkill = ron::from_str(r#"(skill: Slug("panic"), hm_only: true)"#).unwrap();
        assert!(skill.hm_only);

        let normal: FoeSkill = ron::from_str(r#"(skill: Slug("panic"))"#).unwrap();
        assert!(!normal.hm_only);
    }
}
