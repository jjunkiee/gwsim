//! The ten conditions.

use serde::{Deserialize, Serialize};

/// One of the game's ten conditions.
///
/// Conditions never scale with attribute rank: the effect is the same however
/// it was applied, and only the duration varies. That is why the numbers live
/// in `core/conditions.ron` as fixed values rather than as scaled ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Condition {
    Bleeding,
    Blind,
    Burning,
    CrackedArmor,
    Crippled,
    Dazed,
    DeepWound,
    Disease,
    Poison,
    Weakness,
}

impl Condition {
    /// Every condition.
    pub const ALL: [Condition; 10] = [
        Condition::Bleeding,
        Condition::Blind,
        Condition::Burning,
        Condition::CrackedArmor,
        Condition::Crippled,
        Condition::Dazed,
        Condition::DeepWound,
        Condition::Disease,
        Condition::Poison,
        Condition::Weakness,
    ];

    /// This condition's position in [`Self::ALL`].
    ///
    /// Used to index the lookup tables in `CoreData`, which is why it is
    /// total: every variant has one, so an indexed lookup cannot fail.
    pub fn index(self) -> usize {
        self as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_ten_conditions() {
        assert_eq!(Condition::ALL.len(), 10);
    }
    #[test]
    fn index_matches_position_in_all() {
        for (position, value) in Condition::ALL.iter().enumerate() {
            assert_eq!(value.index(), position, "{value:?} is out of place");
        }
    }
}
