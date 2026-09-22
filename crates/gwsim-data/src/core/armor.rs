//! Armor slots and the bonuses that sit on them.

use serde::{Deserialize, Serialize};

use super::DamageType;

/// One of the five armor pieces a character wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ArmorSlot {
    Head,
    Chest,
    Hands,
    Legs,
    Feet,
}

impl ArmorSlot {
    /// Every armor slot.
    pub const ALL: [ArmorSlot; 5] = [
        ArmorSlot::Head,
        ArmorSlot::Chest,
        ArmorSlot::Hands,
        ArmorSlot::Legs,
        ArmorSlot::Feet,
    ];

    /// This slot's position in [`Self::ALL`].
    pub fn index(self) -> usize {
        self as usize
    }
}

/// A family of damage types an armor bonus can name.
///
/// Only two professions have one, and both name a whole family rather than a
/// single type: the Warrior's bonus covers all physical damage and the
/// Ranger's covers all elemental damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DamageClass {
    Physical,
    Elemental,
}

impl DamageClass {
    /// Whether a damage type belongs to this class.
    pub fn covers(self, damage_type: DamageType) -> bool {
        match self {
            DamageClass::Physical => damage_type.is_physical(),
            DamageClass::Elemental => damage_type.is_elemental(),
        }
    }
}

/// Extra armor against a family of damage types, on every piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmorClassBonus {
    /// The damage family the bonus applies to.
    pub against: DamageClass,
    /// How much armor it adds.
    pub amount: i16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_matches_position_in_all() {
        for (position, value) in ArmorSlot::ALL.iter().enumerate() {
            assert_eq!(value.index(), position, "{value:?} is out of place");
        }
    }

    #[test]
    fn damage_classes_cover_the_right_types() {
        assert!(DamageClass::Physical.covers(DamageType::Slashing));
        assert!(!DamageClass::Physical.covers(DamageType::Fire));
        assert!(DamageClass::Elemental.covers(DamageType::Fire));
        assert!(!DamageClass::Elemental.covers(DamageType::Holy));
        // Holy damage bypasses both professions' bonuses, which is the point
        // of it.
        assert!(!DamageClass::Physical.covers(DamageType::Holy));
    }
}
