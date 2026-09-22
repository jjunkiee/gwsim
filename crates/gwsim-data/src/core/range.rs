//! Named range bands.

use serde::{Deserialize, Serialize};

/// A named distance band, measured in gwinches.
///
/// The gwinch value of each band lives in `core/ranges.ron`, not here: this
/// enum is the *name*, and [`crate::core::Ranges`] resolves it to a number.
///
/// Variants are declared shortest first, so the derived [`Ord`] orders them by
/// distance.
///
/// **"Half range" is deliberately absent.** It means "half of whatever this
/// skill's normal range is", so it is a modifier on a band rather than a band
/// of its own, and giving it a variant would put a meaningless absolute
/// number in the data. Skills that are half-ranged carry a flag instead.
///
/// **"Adjacent to target" is also absent.** It is roughly melee reach and is
/// the scythe radius, but the wiki publishes no gwinch value for it, so it is
/// not something `ranges.ron` can hold. Skills needing it use [`Self::Touch`]
/// until WP4.2 settles a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RangeBand {
    /// Melee attacks and touch skills.
    Touch,
    /// The "adjacent" area of effect, and self-targeted skills (A-024).
    Adjacent,
    /// The slightly smaller area a handful of skills use instead of
    /// [`Self::Nearby`] (A-025).
    Aoe240,
    /// "Nearby", "near the target".
    Nearby,
    /// "In this location". Wards and wells.
    InTheArea,
    /// Spear and shortbow range.
    Spear,
    /// The aggro bubble. Most shouts and chants.
    Earshot,
    /// The default for targeted non-attack skills, and for staves, wands and
    /// projectile spells.
    Casting,
    /// Hornbow and recurve bow range.
    Hornbow,
    /// Longbow and flatbow range. Spirit attacks also use it.
    Longbow,
    /// Binding rituals and most passive spirits.
    SpiritRange,
    /// Nature rituals, since the 2026-08-26 update (A-018).
    NatureRitual,
    /// Party-area skills. Also the range at which maintained enchantments
    /// drop.
    Party,
}

impl RangeBand {
    /// Every band, shortest first.
    pub const ALL: [RangeBand; 13] = [
        RangeBand::Touch,
        RangeBand::Adjacent,
        RangeBand::Aoe240,
        RangeBand::Nearby,
        RangeBand::InTheArea,
        RangeBand::Spear,
        RangeBand::Earshot,
        RangeBand::Casting,
        RangeBand::Hornbow,
        RangeBand::Longbow,
        RangeBand::SpiritRange,
        RangeBand::NatureRitual,
        RangeBand::Party,
    ];

    /// This band's position in [`Self::ALL`].
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
    fn declaration_order_is_shortest_first() {
        for pair in RangeBand::ALL.windows(2) {
            assert!(
                pair[0] < pair[1],
                "{:?} should sort before {:?}",
                pair[0],
                pair[1]
            );
        }
    }
    #[test]
    fn index_matches_position_in_all() {
        for (position, value) in RangeBand::ALL.iter().enumerate() {
            assert_eq!(value.index(), position, "{value:?} is out of place");
        }
    }
}
