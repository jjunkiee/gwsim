//! The 42 attributes, and the inherent effects some of them carry.

use serde::{Deserialize, Serialize};

use super::Profession;

/// One of the game's 42 attributes.
///
/// Variants are declared in template-id order, so the derived [`Ord`] matches
/// the order the game's template format uses and keeps `BTreeMap<Attribute, _>`
/// iteration deterministic.
///
/// **Template ids 26, 27 and 28 are unused.** They have no variant, and
/// [`Attribute::from_template_id`] returns [`None`] for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Attribute {
    // Mesmer
    FastCasting,
    IllusionMagic,
    DominationMagic,
    InspirationMagic,
    // Necromancer
    BloodMagic,
    DeathMagic,
    SoulReaping,
    Curses,
    // Elementalist
    AirMagic,
    EarthMagic,
    FireMagic,
    WaterMagic,
    EnergyStorage,
    // Monk
    HealingPrayers,
    SmitingPrayers,
    ProtectionPrayers,
    DivineFavor,
    // Warrior
    Strength,
    AxeMastery,
    HammerMastery,
    Swordsmanship,
    Tactics,
    // Ranger
    BeastMastery,
    Expertise,
    WildernessSurvival,
    Marksmanship,
    // Assassin (ids 29 to 31; 26 to 28 are unused)
    DaggerMastery,
    DeadlyArts,
    ShadowArts,
    // Ritualist
    Communing,
    RestorationMagic,
    ChannelingMagic,
    // Assassin and Ritualist primaries, out of profession order in the format
    CriticalStrikes,
    SpawningPower,
    // Paragon
    SpearMastery,
    Command,
    Motivation,
    Leadership,
    // Dervish
    ScytheMastery,
    WindPrayers,
    EarthPrayers,
    Mysticism,
}

impl Attribute {
    /// Every attribute, in template-id order.
    pub const ALL: [Attribute; 42] = [
        Attribute::FastCasting,
        Attribute::IllusionMagic,
        Attribute::DominationMagic,
        Attribute::InspirationMagic,
        Attribute::BloodMagic,
        Attribute::DeathMagic,
        Attribute::SoulReaping,
        Attribute::Curses,
        Attribute::AirMagic,
        Attribute::EarthMagic,
        Attribute::FireMagic,
        Attribute::WaterMagic,
        Attribute::EnergyStorage,
        Attribute::HealingPrayers,
        Attribute::SmitingPrayers,
        Attribute::ProtectionPrayers,
        Attribute::DivineFavor,
        Attribute::Strength,
        Attribute::AxeMastery,
        Attribute::HammerMastery,
        Attribute::Swordsmanship,
        Attribute::Tactics,
        Attribute::BeastMastery,
        Attribute::Expertise,
        Attribute::WildernessSurvival,
        Attribute::Marksmanship,
        Attribute::DaggerMastery,
        Attribute::DeadlyArts,
        Attribute::ShadowArts,
        Attribute::Communing,
        Attribute::RestorationMagic,
        Attribute::ChannelingMagic,
        Attribute::CriticalStrikes,
        Attribute::SpawningPower,
        Attribute::SpearMastery,
        Attribute::Command,
        Attribute::Motivation,
        Attribute::Leadership,
        Attribute::ScytheMastery,
        Attribute::WindPrayers,
        Attribute::EarthPrayers,
        Attribute::Mysticism,
    ];

    /// This attribute's position in [`Self::ALL`].
    ///
    /// Used to index the lookup tables in `CoreData`, which is why it is
    /// total: every variant has one, so an indexed lookup cannot fail.
    pub fn index(self) -> usize {
        self as usize
    }

    /// Template ids that exist in the format but name no attribute.
    pub const UNUSED_TEMPLATE_IDS: [u8; 3] = [26, 27, 28];

    /// The highest template id in use.
    pub const MAX_TEMPLATE_ID: u8 = 44;

    /// The id this attribute has in a skill template code.
    pub fn template_id(self) -> u8 {
        match self {
            Attribute::FastCasting => 0,
            Attribute::IllusionMagic => 1,
            Attribute::DominationMagic => 2,
            Attribute::InspirationMagic => 3,
            Attribute::BloodMagic => 4,
            Attribute::DeathMagic => 5,
            Attribute::SoulReaping => 6,
            Attribute::Curses => 7,
            Attribute::AirMagic => 8,
            Attribute::EarthMagic => 9,
            Attribute::FireMagic => 10,
            Attribute::WaterMagic => 11,
            Attribute::EnergyStorage => 12,
            Attribute::HealingPrayers => 13,
            Attribute::SmitingPrayers => 14,
            Attribute::ProtectionPrayers => 15,
            Attribute::DivineFavor => 16,
            Attribute::Strength => 17,
            Attribute::AxeMastery => 18,
            Attribute::HammerMastery => 19,
            Attribute::Swordsmanship => 20,
            Attribute::Tactics => 21,
            Attribute::BeastMastery => 22,
            Attribute::Expertise => 23,
            Attribute::WildernessSurvival => 24,
            Attribute::Marksmanship => 25,
            Attribute::DaggerMastery => 29,
            Attribute::DeadlyArts => 30,
            Attribute::ShadowArts => 31,
            Attribute::Communing => 32,
            Attribute::RestorationMagic => 33,
            Attribute::ChannelingMagic => 34,
            Attribute::CriticalStrikes => 35,
            Attribute::SpawningPower => 36,
            Attribute::SpearMastery => 37,
            Attribute::Command => 38,
            Attribute::Motivation => 39,
            Attribute::Leadership => 40,
            Attribute::ScytheMastery => 41,
            Attribute::WindPrayers => 42,
            Attribute::EarthPrayers => 43,
            Attribute::Mysticism => 44,
        }
    }

    /// The attribute with this template id.
    ///
    /// Returns [`None`] for the unused ids 26 to 28 and for anything above 44,
    /// which is what makes a corrupt template code fail loudly rather than
    /// decode into the wrong attribute.
    pub fn from_template_id(id: u8) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|attribute| attribute.template_id() == id)
    }

    /// The profession this attribute belongs to.
    pub fn profession(self) -> Profession {
        match self {
            Attribute::FastCasting
            | Attribute::IllusionMagic
            | Attribute::DominationMagic
            | Attribute::InspirationMagic => Profession::Mesmer,
            Attribute::BloodMagic
            | Attribute::DeathMagic
            | Attribute::SoulReaping
            | Attribute::Curses => Profession::Necromancer,
            Attribute::AirMagic
            | Attribute::EarthMagic
            | Attribute::FireMagic
            | Attribute::WaterMagic
            | Attribute::EnergyStorage => Profession::Elementalist,
            Attribute::HealingPrayers
            | Attribute::SmitingPrayers
            | Attribute::ProtectionPrayers
            | Attribute::DivineFavor => Profession::Monk,
            Attribute::Strength
            | Attribute::AxeMastery
            | Attribute::HammerMastery
            | Attribute::Swordsmanship
            | Attribute::Tactics => Profession::Warrior,
            Attribute::BeastMastery
            | Attribute::Expertise
            | Attribute::WildernessSurvival
            | Attribute::Marksmanship => Profession::Ranger,
            Attribute::DaggerMastery
            | Attribute::DeadlyArts
            | Attribute::ShadowArts
            | Attribute::CriticalStrikes => Profession::Assassin,
            Attribute::Communing
            | Attribute::RestorationMagic
            | Attribute::ChannelingMagic
            | Attribute::SpawningPower => Profession::Ritualist,
            Attribute::SpearMastery
            | Attribute::Command
            | Attribute::Motivation
            | Attribute::Leadership => Profession::Paragon,
            Attribute::ScytheMastery
            | Attribute::WindPrayers
            | Attribute::EarthPrayers
            | Attribute::Mysticism => Profession::Dervish,
        }
    }

    /// Whether this is its profession's primary attribute.
    ///
    /// Only a character whose *primary* profession owns the attribute gets its
    /// inherent effect, and only primary-profession attributes can take runes.
    pub fn is_primary(self) -> bool {
        self.profession().primary_attribute() == self
    }
}

/// An attribute's built-in effect, which applies whatever skills are equipped.
///
/// The engine dispatches on this rather than on the attribute itself, so a
/// rule is written once even when several attributes share it (the seven
/// weapon masteries all scale weapon damage the same way).
///
/// **Having an inherent effect is not the same as being primary.** Beast
/// Mastery, Dagger Mastery, Death Magic and the weapon masteries all carry one
/// without being primary attributes, so anything that needs "does this do
/// something on its own?" must ask for this tag, not for
/// [`Attribute::is_primary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum InherentEffect {
    /// Warrior: armor penetration on attack skills.
    ArmorPenetration,
    /// Ranger: cheaper Ranger, attack, touch and ritual skills.
    Expertise,
    /// Monk: bonus healing when a Monk spell targets an ally.
    DivineFavor,
    /// Necromancer: energy when a creature dies nearby.
    SoulReaping,
    /// Mesmer: faster spell and signet activation, and in PvE shorter Mesmer
    /// spell recharges.
    FastCasting,
    /// Elementalist: more maximum energy per rank.
    EnergyStorage,
    /// Assassin: higher critical chance, and energy on a critical hit.
    CriticalStrikes,
    /// Ritualist: more health for created creatures, and longer weapon spells.
    SpawningPower,
    /// Paragon: energy from shouts and chants that affect allies.
    Leadership,
    /// Dervish: cheaper Dervish enchantments, and armor while enchanted.
    Mysticism,
    /// Necromancer's Death Magic: raises the minion cap.
    MinionCap,
    /// Ranger's Beast Mastery: pet damage and critical chance.
    PetMastery,
    /// Assassin's Dagger Mastery: chance of a double strike, on top of the
    /// weapon-mastery effect every martial attribute has.
    DoubleStrike,
    /// The seven martial attributes: weapon damage and critical chance.
    WeaponMastery,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_forty_two_attributes() {
        assert_eq!(Attribute::ALL.len(), 42);
    }

    #[test]
    fn template_ids_round_trip() {
        for attribute in Attribute::ALL {
            let id = attribute.template_id();
            assert_eq!(
                Attribute::from_template_id(id),
                Some(attribute),
                "{attribute:?} did not survive a round trip through id {id}"
            );
        }
    }

    #[test]
    fn template_ids_cover_zero_to_twenty_five_and_twenty_nine_to_forty_four() {
        let mut ids: Vec<u8> = Attribute::ALL
            .iter()
            .map(|attribute| attribute.template_id())
            .collect();
        ids.sort_unstable();

        let expected: Vec<u8> = (0..=25).chain(29..=44).collect();
        assert_eq!(ids, expected);
    }

    #[test]
    fn unused_template_ids_have_no_attribute() {
        // 26 to 28 exist in the format's numbering but name nothing. A template
        // code carrying one is corrupt, and must not silently decode.
        for id in Attribute::UNUSED_TEMPLATE_IDS {
            assert_eq!(
                Attribute::from_template_id(id),
                None,
                "unused id {id} decoded to an attribute"
            );
        }
        assert_eq!(Attribute::from_template_id(45), None);
        assert_eq!(Attribute::from_template_id(255), None);
    }

    #[test]
    fn declaration_order_matches_template_id_order() {
        // The derived Ord is relied on for deterministic BTreeMap iteration, so
        // it has to agree with the template numbering.
        for pair in Attribute::ALL.windows(2) {
            assert!(
                pair[0].template_id() < pair[1].template_id(),
                "{:?} and {:?} are declared out of template-id order",
                pair[0],
                pair[1]
            );
            assert!(pair[0] < pair[1]);
        }
    }

    #[test]
    fn exactly_ten_attributes_are_primary() {
        let primaries: Vec<Attribute> = Attribute::ALL
            .into_iter()
            .filter(|attribute| attribute.is_primary())
            .collect();
        assert_eq!(primaries.len(), 10, "expected one primary per profession");

        for profession in Profession::ALL {
            assert!(
                primaries.contains(&profession.primary_attribute()),
                "{profession:?}'s primary is missing"
            );
        }
    }

    #[test]
    fn primary_attribute_ids_are_not_contiguous() {
        // Guards against anyone "simplifying" the primary check into an id
        // range. Critical Strikes (35) and Spawning Power (36) sit outside
        // their professions' other attributes, which is why the format's
        // attribute order is not the profession order.
        assert_eq!(Attribute::CriticalStrikes.template_id(), 35);
        assert_eq!(
            Attribute::CriticalStrikes.profession(),
            Profession::Assassin
        );
        assert_eq!(Attribute::DaggerMastery.template_id(), 29);
        assert_eq!(Attribute::SpawningPower.template_id(), 36);
        assert_eq!(Attribute::SpawningPower.profession(), Profession::Ritualist);
        assert_eq!(Attribute::Communing.template_id(), 32);
    }
    #[test]
    fn index_matches_position_in_all() {
        for (position, value) in Attribute::ALL.iter().enumerate() {
            assert_eq!(value.index(), position, "{value:?} is out of place");
        }
    }
}
