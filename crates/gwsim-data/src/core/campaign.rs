//! Campaigns and title tracks.

use serde::{Deserialize, Serialize};

/// The campaign a skill, profession or area comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Campaign {
    /// Available whatever campaigns the account owns.
    Core,
    Prophecies,
    Factions,
    Nightfall,
    EyeOfTheNorth,
}

impl Campaign {
    /// Every campaign.
    pub const ALL: [Campaign; 5] = [
        Campaign::Core,
        Campaign::Prophecies,
        Campaign::Factions,
        Campaign::Nightfall,
        Campaign::EyeOfTheNorth,
    ];

    /// This campaign's position in [`Self::ALL`].
    ///
    /// Used to index the lookup tables in `CoreData`, which is why it is
    /// total: every variant has one, so an indexed lookup cannot fail.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// A title track that PvE-only skills scale on.
///
/// The rank-to-effective-rank table for each track lives in `core/titles.ron`
/// (A-020). Only Asura is needed for M1 — Air of Superiority is the one M1
/// skill that scales on a title — and WP5.7 and WP7.5 add the rest.
///
/// **Not every title-scaled skill fits this shape.** *Sunspear Rebirth Signet*
/// and Lightbringer effects scale linearly with rank rather than through a
/// lookup table, so they will need a second mechanism when they arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TitleTrack {
    Asura,
    Deldrimor,
    EbonVanguard,
    Norn,
    Sunspear,
    Lightbringer,
    Kurzick,
    Luxon,
}

impl TitleTrack {
    /// Every title track that sets PvE-only skill values (T5.7.1).
    pub const ALL: [TitleTrack; 8] = [
        TitleTrack::Asura,
        TitleTrack::Deldrimor,
        TitleTrack::EbonVanguard,
        TitleTrack::Norn,
        TitleTrack::Sunspear,
        TitleTrack::Lightbringer,
        TitleTrack::Kurzick,
        TitleTrack::Luxon,
    ];

    /// This track's position in [`Self::ALL`].
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
    fn campaign_index_matches_position_in_all() {
        for (position, value) in Campaign::ALL.iter().enumerate() {
            assert_eq!(value.index(), position, "{value:?} is out of place");
        }
    }

    #[test]
    fn title_track_index_matches_position_in_all() {
        for (position, value) in TitleTrack::ALL.iter().enumerate() {
            assert_eq!(value.index(), position, "{value:?} is out of place");
        }
    }
}
