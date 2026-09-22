//! The shapes of the `data/items/*.ron` files (§7.1).

use serde::{Deserialize, Serialize};

use crate::core::{ArmorSlot, Attribute, DamageType, Profession};
use crate::dsl;
use crate::ids::Slug;
use crate::provenance::Provenance;
use crate::units::Seconds;

// --------------------------------------------------------------------- armor

/// `data/items/armor.ron`: base armor rating per profession and piece.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmorFile {
    pub provenance: Provenance,
    pub pieces: Vec<ArmorPieceBase>,
}

/// The armor rating one piece of a profession's maximum-rating armor gives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmorPieceBase {
    pub profession: Profession,
    pub slot: ArmorSlot,
    pub armor: i16,
}

// --------------------------------------------------------------------- runes

/// `data/items/runes.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunesFile {
    pub provenance: Provenance,
    pub runes: Vec<Rune>,
}

/// A rune.
///
/// Two rules make runes awkward, and both are encoded here rather than left to
/// the caller: **only the best rune of a kind counts**, which is what
/// `non_stacking_key` decides, while **every rune's health penalty applies**,
/// stacking or not. Losing the second rule would quietly overstate health on
/// any build running two attribute runes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rune {
    pub slug: Slug,
    pub name: String,
    pub kind: RuneKind,
    /// What the rune grants.
    #[serde(default)]
    pub bonus: Option<RuneBonus>,
    /// Maximum health lost, as a positive number. Applies even when the rune's
    /// bonus is suppressed by a better one.
    #[serde(default)]
    pub health_penalty: u16,
    /// Runes sharing a key do not stack; only the strongest applies.
    pub non_stacking_key: String,
    /// The id this rune has in an equipment template code.
    #[serde(default)]
    pub template_modifier_id: Option<u32>,
}

/// What family a rune belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuneKind {
    /// Raises one attribute. Primary-profession attributes only.
    Attribute {
        attribute: Attribute,
        tier: RuneTier,
    },
    /// Raises maximum health.
    Vigor(RuneTier),
    /// Raises maximum health by a small fixed amount.
    Vitae,
    /// Raises maximum energy.
    Attunement,
    /// Shortens Blind and Weakness.
    Clarity,
    /// Shortens Disease and Poison.
    Purity,
    /// Shortens Dazed and Deep Wound.
    Recovery,
    /// Shortens Bleeding and Crippled.
    Restoration,
}

/// A rune's grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RuneTier {
    Minor,
    Major,
    Superior,
}

/// What a rune grants.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RuneBonus {
    /// Ranks added to an attribute.
    AttributeRank(u8),
    /// Maximum health added.
    MaxHealth(u16),
    /// Maximum energy added.
    MaxEnergy(u16),
    /// Condition duration removed, as a percentage.
    ConditionReduction { percent: u8 },
}

// ----------------------------------------------------------------- insignias

/// `data/items/insignias.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InsigniasFile {
    pub provenance: Provenance,
    pub insignias: Vec<Insignia>,
}

/// An insignia.
///
/// **Insignia armor applies only to the piece it sits on** (§7.1), which is
/// why `per_piece` exists and defaults to true: an insignia bonus read as
/// global would multiply a build's armor by five.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Insignia {
    pub slug: Slug,
    pub name: String,
    /// The profession whose armor it fits. [`None`] for universal insignias.
    #[serde(default)]
    pub profession: Option<Profession>,
    /// What it does. The DSL types are placeholders until WP1.5.
    #[serde(default)]
    pub effects: Vec<dsl::Action>,
    /// Whether the effect counts once per piece or once for the whole set.
    #[serde(default = "yes")]
    pub per_piece: bool,
    #[serde(default)]
    pub template_modifier_id: Option<u32>,
}

fn yes() -> bool {
    true
}

// ------------------------------------------------------------------- weapons

/// `data/items/weapons.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponsFile {
    pub provenance: Provenance,
    pub weapons: Vec<WeaponType>,
}

/// A kind of weapon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponType {
    pub slug: Slug,
    pub name: String,
    /// The damage it deals. [`None`] for items that do not attack.
    #[serde(default)]
    pub damage_type: Option<DamageType>,
    /// Minimum and maximum damage at the requirement it asks for.
    #[serde(default)]
    pub damage_range_at_max_req: Option<(u16, u16)>,
    #[serde(default)]
    pub attack_interval: Option<Seconds>,
    /// How far it reaches, in gwinches.
    #[serde(default)]
    pub range: Option<f32>,
    /// How fast its projectile travels. A-003: not documented per item.
    #[serde(default)]
    pub projectile_speed: Option<f32>,
    /// The attribute that scales its damage.
    #[serde(default)]
    pub mastery: Option<Attribute>,
    #[serde(default)]
    pub two_handed: bool,
    /// Maximum energy the item itself grants.
    #[serde(default)]
    pub energy: Option<u8>,
    #[serde(default)]
    pub template_item_id: Option<u32>,
}

// ----------------------------------------------------------- weapon upgrades

/// `data/items/weapon_upgrades.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponUpgradesFile {
    pub provenance: Provenance,
    pub upgrades: Vec<WeaponUpgrade>,
}

/// A component fitted to a weapon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponUpgrade {
    pub slug: Slug,
    pub name: String,
    pub slot: UpgradeSlot,
    #[serde(default)]
    pub effects: Vec<dsl::Action>,
    /// When the effect applies, if not always.
    #[serde(default)]
    pub conditions: Vec<dsl::Value>,
    #[serde(default)]
    pub template_modifier_id: Option<u32>,
}

/// Where an upgrade fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UpgradeSlot {
    Prefix,
    Suffix,
    Inscription,
    /// Built into the item rather than fitted.
    Inherent,
}

// --------------------------------------------------------------- consumables

/// `data/items/consumables.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumablesFile {
    pub provenance: Provenance,
    pub consumables: Vec<Consumable>,
}

/// Something eaten or drunk for a timed effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consumable {
    pub slug: Slug,
    pub name: String,
    #[serde(default)]
    pub effects: Vec<dsl::Action>,
    #[serde(default)]
    pub duration: Option<Seconds>,
    pub scope: ConsumableScope,
    /// Whether the effect survives the user's death.
    ///
    /// A-019: the item pages and the Death page disagree, so this is recorded
    /// per item and the assumption is referenced in provenance.
    #[serde(default)]
    pub survives_death: bool,
}

/// Who a consumable affects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsumableScope {
    SelfOnly,
    Party,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(text: &str) -> T
    where
        T: serde::de::DeserializeOwned + Serialize + PartialEq + std::fmt::Debug,
    {
        let original: T = ron::from_str(text).expect("should parse");
        let written = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("should serialise");
        let back: T = ron::from_str(&written).expect("should parse again");
        assert_eq!(back, original, "round trip changed the value");
        original
    }

    #[test]
    fn an_attribute_rune_round_trips() {
        let rune: Rune = round_trip(
            r#"(
                slug: "superior-domination-magic",
                name: "Superior Rune of Domination Magic",
                kind: Attribute(attribute: DominationMagic, tier: Superior),
                bonus: Some(AttributeRank(3)),
                health_penalty: 75,
                non_stacking_key: "attribute:DominationMagic",
            )"#,
        );
        assert_eq!(rune.health_penalty, 75);
        assert_eq!(rune.bonus, Some(RuneBonus::AttributeRank(3)));
    }

    #[test]
    fn the_three_attribute_rune_tiers_have_the_wikis_penalties() {
        // Minor 0, major 35, superior 75. Getting these wrong shifts every
        // caster build's health by up to 150.
        for (tier, rank, penalty) in [
            (RuneTier::Minor, 1u8, 0u16),
            (RuneTier::Major, 2, 35),
            (RuneTier::Superior, 3, 75),
        ] {
            let rune = Rune {
                slug: "x".parse().unwrap(),
                name: String::new(),
                kind: RuneKind::Attribute {
                    attribute: Attribute::DominationMagic,
                    tier,
                },
                bonus: Some(RuneBonus::AttributeRank(rank)),
                health_penalty: penalty,
                non_stacking_key: "attribute:DominationMagic".to_owned(),
                template_modifier_id: None,
            };
            assert_eq!(rune.health_penalty, penalty, "{tier:?}");
        }
    }

    #[test]
    fn runes_of_the_same_family_share_a_non_stacking_key() {
        // Two Vigor runes must collapse to the best one, so they have to agree
        // on the key. Two different attributes must not.
        let vigor = |tier| Rune {
            slug: "x".parse().unwrap(),
            name: String::new(),
            kind: RuneKind::Vigor(tier),
            bonus: Some(RuneBonus::MaxHealth(50)),
            health_penalty: 0,
            non_stacking_key: "vigor".to_owned(),
            template_modifier_id: None,
        };
        assert_eq!(
            vigor(RuneTier::Minor).non_stacking_key,
            vigor(RuneTier::Superior).non_stacking_key
        );
    }

    #[test]
    fn an_insignia_is_per_piece_unless_it_says_otherwise() {
        // The default that matters: reading a per-piece armor bonus as global
        // would quintuple it.
        let insignia: Insignia = ron::from_str(
            r#"(slug: "prodigys", name: "Prodigy's Insignia", profession: Some(Mesmer))"#,
        )
        .expect("should parse");
        assert!(insignia.per_piece);

        let global: Insignia =
            ron::from_str(r#"(slug: "x", name: "X", per_piece: false)"#).expect("should parse");
        assert!(!global.per_piece);
    }

    #[test]
    fn a_weapon_type_round_trips() {
        let staff: WeaponType = round_trip(
            r#"(
                slug: "staff",
                name: "Staff",
                damage_type: Some(Blunt),
                damage_range_at_max_req: Some((11, 22)),
                attack_interval: Some(1.75),
                range: Some(1248.0),
                two_handed: true,
                energy: Some(10),
            )"#,
        );
        assert!(staff.two_handed);
        assert_eq!(staff.attack_interval.unwrap().ms(), 1750);
        assert_eq!(staff.energy, Some(10));
    }

    #[test]
    fn a_weapon_without_a_mastery_is_allowed() {
        // Wands and staves scale on whichever caster attribute they require,
        // which is a property of the individual item rather than the type.
        let wand: WeaponType =
            ron::from_str(r#"(slug: "wand", name: "Wand")"#).expect("should parse");
        assert_eq!(wand.mastery, None);
        assert_eq!(wand.damage_type, None);
    }

    #[test]
    fn the_four_upgrade_slots_round_trip() {
        for slot in ["Prefix", "Suffix", "Inscription", "Inherent"] {
            let text = format!(r#"(slug: "x", name: "X", slot: {slot})"#);
            let upgrade: WeaponUpgrade = ron::from_str(&text).expect("should parse");
            assert_eq!(ron::to_string(&upgrade.slot).unwrap(), slot);
        }
    }

    #[test]
    fn a_consumable_records_whether_it_survives_death() {
        // A-019 is a live wiki conflict, so the field must be explicit rather
        // than inferred.
        let consumable: Consumable = round_trip(
            r#"(
                slug: "grail-of-might",
                name: "Grail of Might",
                duration: Some(1800.0),
                scope: Party,
                survives_death: true,
            )"#,
        );
        assert!(consumable.survives_death);
        assert_eq!(consumable.scope, ConsumableScope::Party);
        assert_eq!(consumable.duration.unwrap().ms(), 1_800_000);
    }

    #[test]
    fn armor_base_rows_round_trip() {
        let file: ArmorFile = round_trip(
            r#"(
                provenance: (
                    sources: ["https://wiki.guildwars.com/wiki/Armor_rating"],
                    crawled: "2026-09-22",
                    review: Draft,
                ),
                pieces: [
                    (profession: Mesmer, slot: Head, armor: 12),
                    (profession: Mesmer, slot: Chest, armor: 12),
                ],
            )"#,
        );
        assert_eq!(file.pieces.len(), 2);
    }

    #[test]
    fn a_misspelled_item_field_is_rejected() {
        assert!(ron::from_str::<WeaponType>(r#"(slug: "x", name: "X", twohanded: true)"#).is_err());
    }
}
