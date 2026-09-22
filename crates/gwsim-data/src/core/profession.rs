//! The ten professions and the attributes each one owns.

use serde::{Deserialize, Serialize};

use super::Attribute;

/// One of the game's ten professions.
///
/// Variants are declared in template-index order, so the derived [`Ord`]
/// matches the order the game's template format uses.
///
/// A character's secondary profession is modelled as `Option<Profession>`;
/// template index 0 ("no profession") is [`None`] rather than a variant, so
/// the type never holds a profession that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Profession {
    Warrior,
    Ranger,
    Monk,
    Necromancer,
    Mesmer,
    Elementalist,
    Assassin,
    Ritualist,
    Paragon,
    Dervish,
}

impl Profession {
    /// Every profession, in template-index order.
    pub const ALL: [Profession; 10] = [
        Profession::Warrior,
        Profession::Ranger,
        Profession::Monk,
        Profession::Necromancer,
        Profession::Mesmer,
        Profession::Elementalist,
        Profession::Assassin,
        Profession::Ritualist,
        Profession::Paragon,
        Profession::Dervish,
    ];

    /// This profession's position in [`Self::ALL`].
    ///
    /// Used to index the lookup tables in `CoreData`, which is why it is
    /// total: every variant has one, so an indexed lookup cannot fail.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The index this profession has in a skill template code (1 to 10).
    ///
    /// Index 0 means "no profession" and has no variant here.
    pub fn template_index(self) -> u8 {
        match self {
            Profession::Warrior => 1,
            Profession::Ranger => 2,
            Profession::Monk => 3,
            Profession::Necromancer => 4,
            Profession::Mesmer => 5,
            Profession::Elementalist => 6,
            Profession::Assassin => 7,
            Profession::Ritualist => 8,
            Profession::Paragon => 9,
            Profession::Dervish => 10,
        }
    }

    /// The profession with this template index, or [`None`] for 0 ("no
    /// profession") and for any index above 10.
    pub fn from_template_index(index: u8) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|profession| profession.template_index() == index)
    }

    /// The short form the wiki uses, such as `Me` for Mesmer.
    ///
    /// Monk, Mesmer and Ritualist take two letters because `M` and `R` alone
    /// would be ambiguous.
    pub fn abbrev(self) -> &'static str {
        match self {
            Profession::Warrior => "W",
            Profession::Ranger => "R",
            Profession::Monk => "Mo",
            Profession::Necromancer => "N",
            Profession::Mesmer => "Me",
            Profession::Elementalist => "E",
            Profession::Assassin => "A",
            Profession::Ritualist => "Rt",
            Profession::Paragon => "P",
            Profession::Dervish => "D",
        }
    }

    /// This profession's primary attribute.
    ///
    /// A character only gets the inherent effect of its *primary* profession's
    /// primary attribute; taking the profession as a secondary does not grant
    /// it.
    pub fn primary_attribute(self) -> Attribute {
        match self {
            Profession::Warrior => Attribute::Strength,
            Profession::Ranger => Attribute::Expertise,
            Profession::Monk => Attribute::DivineFavor,
            Profession::Necromancer => Attribute::SoulReaping,
            Profession::Mesmer => Attribute::FastCasting,
            Profession::Elementalist => Attribute::EnergyStorage,
            Profession::Assassin => Attribute::CriticalStrikes,
            Profession::Ritualist => Attribute::SpawningPower,
            Profession::Paragon => Attribute::Leadership,
            Profession::Dervish => Attribute::Mysticism,
        }
    }

    /// Every attribute of this profession, primary included, in template-id
    /// order.
    ///
    /// Warrior and Elementalist have five; the rest have four.
    pub fn attributes(self) -> &'static [Attribute] {
        use Attribute as A;
        match self {
            Profession::Warrior => &[
                A::Strength,
                A::AxeMastery,
                A::HammerMastery,
                A::Swordsmanship,
                A::Tactics,
            ],
            Profession::Ranger => &[
                A::BeastMastery,
                A::Expertise,
                A::WildernessSurvival,
                A::Marksmanship,
            ],
            Profession::Monk => &[
                A::HealingPrayers,
                A::SmitingPrayers,
                A::ProtectionPrayers,
                A::DivineFavor,
            ],
            Profession::Necromancer => &[A::BloodMagic, A::DeathMagic, A::SoulReaping, A::Curses],
            Profession::Mesmer => &[
                A::FastCasting,
                A::IllusionMagic,
                A::DominationMagic,
                A::InspirationMagic,
            ],
            Profession::Elementalist => &[
                A::AirMagic,
                A::EarthMagic,
                A::FireMagic,
                A::WaterMagic,
                A::EnergyStorage,
            ],
            Profession::Assassin => &[
                A::DaggerMastery,
                A::DeadlyArts,
                A::ShadowArts,
                A::CriticalStrikes,
            ],
            Profession::Ritualist => &[
                A::Communing,
                A::RestorationMagic,
                A::ChannelingMagic,
                A::SpawningPower,
            ],
            Profession::Paragon => &[A::SpearMastery, A::Command, A::Motivation, A::Leadership],
            Profession::Dervish => &[
                A::ScytheMastery,
                A::WindPrayers,
                A::EarthPrayers,
                A::Mysticism,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_indices_round_trip() {
        for profession in Profession::ALL {
            let index = profession.template_index();
            assert_eq!(
                Profession::from_template_index(index),
                Some(profession),
                "{profession:?} did not survive a round trip through index {index}"
            );
        }
    }

    #[test]
    fn index_matches_position_in_all() {
        for (position, value) in Profession::ALL.iter().enumerate() {
            assert_eq!(value.index(), position, "{value:?} is out of place");
        }
    }

    #[test]
    fn template_indices_are_one_to_ten() {
        let indices: Vec<u8> = Profession::ALL
            .iter()
            .map(|profession| profession.template_index())
            .collect();
        assert_eq!(indices, (1..=10).collect::<Vec<u8>>());
    }

    #[test]
    fn index_zero_is_no_profession() {
        // 0 means "no secondary profession" in a template code. It must not
        // decode to Warrior, which is the mistake a naive 0-based mapping makes.
        assert_eq!(Profession::from_template_index(0), None);
        assert_eq!(Profession::from_template_index(11), None);
        assert_eq!(Profession::from_template_index(255), None);
    }

    #[test]
    fn abbreviations_are_unique() {
        let mut seen: Vec<&str> = Profession::ALL
            .iter()
            .map(|profession| profession.abbrev())
            .collect();
        seen.sort_unstable();
        let count = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), count, "two professions share an abbreviation");
    }

    #[test]
    fn every_attribute_belongs_to_exactly_one_profession() {
        // This is the cross-check between the two halves of the identity graph:
        // Profession::attributes() lists them, Attribute::profession() maps them
        // back. If the two ever disagree, one of them has a typo.
        let mut listed: Vec<Attribute> = Profession::ALL
            .iter()
            .flat_map(|profession| profession.attributes().iter().copied())
            .collect();
        listed.sort_unstable();

        let mut all = Attribute::ALL.to_vec();
        all.sort_unstable();

        assert_eq!(listed, all, "the two attribute listings disagree");

        for profession in Profession::ALL {
            for attribute in profession.attributes() {
                assert_eq!(
                    attribute.profession(),
                    profession,
                    "{attribute:?} is listed under {profession:?} but maps elsewhere"
                );
            }
        }
    }

    #[test]
    fn primary_attribute_is_one_of_the_professions_own() {
        for profession in Profession::ALL {
            let primary = profession.primary_attribute();
            assert!(
                profession.attributes().contains(&primary),
                "{profession:?}'s primary {primary:?} is missing from its attribute list"
            );
        }
    }

    #[test]
    fn warrior_and_elementalist_have_five_attributes() {
        for profession in Profession::ALL {
            let expected = match profession {
                Profession::Warrior | Profession::Elementalist => 5,
                _ => 4,
            };
            assert_eq!(
                profession.attributes().len(),
                expected,
                "{profession:?} has the wrong number of attributes"
            );
        }
    }
}
