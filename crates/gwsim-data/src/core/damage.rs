//! Damage types.

use serde::{Deserialize, Serialize};

/// A damage type.
///
/// Typeless damage — which no armor bonus applies to — is modelled as
/// `Option<DamageType>` rather than a variant, so that "this damage has no
/// type" and "we have not recorded the type yet" cannot be confused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DamageType {
    // Physical
    Blunt,
    Piercing,
    Slashing,
    // Elemental
    Cold,
    Earth,
    Fire,
    Lightning,
    // Neither physical nor elemental
    Chaos,
    Dark,
    Holy,
    Shadow,
}

impl DamageType {
    /// Every damage type.
    pub const ALL: [DamageType; 11] = [
        DamageType::Blunt,
        DamageType::Piercing,
        DamageType::Slashing,
        DamageType::Cold,
        DamageType::Earth,
        DamageType::Fire,
        DamageType::Lightning,
        DamageType::Chaos,
        DamageType::Dark,
        DamageType::Holy,
        DamageType::Shadow,
    ];

    /// This damage type's position in [`Self::ALL`].
    ///
    /// Used to index the lookup tables in `CoreData`, which is why it is
    /// total: every variant has one, so an indexed lookup cannot fail.
    pub fn index(self) -> usize {
        self as usize
    }

    /// Whether a Warrior's inherent "+20 armor vs physical damage" applies.
    pub fn is_physical(self) -> bool {
        matches!(
            self,
            DamageType::Blunt | DamageType::Piercing | DamageType::Slashing
        )
    }

    /// Whether a Ranger's inherent "+30 armor vs elemental damage" applies.
    ///
    /// Only Cold, Earth, Fire and Lightning count. Despite the names, the
    /// Energy Storage, Earth Prayers and Wind Prayers *attributes* are not
    /// "elemental" in this sense, and Chaos, Dark, Holy and Shadow damage are
    /// neither physical nor elemental.
    pub fn is_elemental(self) -> bool {
        matches!(
            self,
            DamageType::Cold | DamageType::Earth | DamageType::Fire | DamageType::Lightning
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_and_elemental_are_disjoint() {
        for damage_type in DamageType::ALL {
            assert!(
                !(damage_type.is_physical() && damage_type.is_elemental()),
                "{damage_type:?} claims to be both physical and elemental"
            );
        }
    }

    #[test]
    fn the_three_physical_types() {
        let physical: Vec<DamageType> = DamageType::ALL
            .into_iter()
            .filter(|damage_type| damage_type.is_physical())
            .collect();
        assert_eq!(
            physical,
            vec![
                DamageType::Blunt,
                DamageType::Piercing,
                DamageType::Slashing
            ]
        );
    }

    #[test]
    fn the_four_elemental_types() {
        let elemental: Vec<DamageType> = DamageType::ALL
            .into_iter()
            .filter(|damage_type| damage_type.is_elemental())
            .collect();
        assert_eq!(
            elemental,
            vec![
                DamageType::Cold,
                DamageType::Earth,
                DamageType::Fire,
                DamageType::Lightning
            ]
        );
    }

    #[test]
    fn four_types_are_neither() {
        // Chaos, Dark, Holy and Shadow bypass both professions' inherent armor
        // bonuses, which is most of why they matter.
        let neither: Vec<DamageType> = DamageType::ALL
            .into_iter()
            .filter(|damage_type| !damage_type.is_physical() && !damage_type.is_elemental())
            .collect();
        assert_eq!(
            neither,
            vec![
                DamageType::Chaos,
                DamageType::Dark,
                DamageType::Holy,
                DamageType::Shadow
            ]
        );
    }
    #[test]
    fn index_matches_position_in_all() {
        for (position, value) in DamageType::ALL.iter().enumerate() {
            assert_eq!(value.index(), position, "{value:?} is out of place");
        }
    }
}
