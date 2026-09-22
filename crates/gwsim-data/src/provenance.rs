//! Where a data file's values came from, and how far they have been checked.

use serde::{Deserialize, Serialize};

use crate::ids::{AssumptionId, IsoDate};

/// How far a file's contents have been checked against the game.
///
/// The order matters: it is the order work happens in, and the derived [`Ord`]
/// lets callers ask for "at least `Draft`".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ReviewStatus {
    /// Seeded from the wiki, with no effect encoding. The evaluator refuses to
    /// run a build containing one of these.
    NumbersOnly,
    /// Encoded, but nobody has checked the encoding against the game.
    Draft,
    /// A person has checked the encoding against the game's behaviour.
    Reviewed,
}

/// The provenance block every entity file carries.
///
/// This is what lets a reader of `data/` tell a checked value from a guess
/// without leaving the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// The wiki pages, or other URLs, the values came from.
    pub sources: Vec<String>,
    /// When those sources were read.
    pub crawled: IsoDate,
    /// How far the file has been checked.
    pub review: ReviewStatus,
    /// Who did the checking. Required once `review` is `Reviewed`.
    #[serde(default)]
    pub reviewed_by: Option<String>,
    /// Assumptions this file relies on.
    #[serde(default)]
    pub assumptions: Vec<AssumptionId>,
    /// Anything a later reader needs to know.
    #[serde(default)]
    pub notes: String,
}

impl Provenance {
    /// Checks the rules that hold for every provenance block, whatever kind of
    /// file it belongs to.
    ///
    /// Returns a message per problem rather than stopping at the first, so one
    /// pass over a file reports everything wrong with it.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();

        if self.sources.is_empty() {
            problems.push(
                "provenance.sources is empty: name the wiki pages these values came from"
                    .to_owned(),
            );
        }
        if self.review == ReviewStatus::Reviewed && self.reviewed_by.is_none() {
            problems.push(
                "provenance.review is Reviewed but reviewed_by is None: a review needs \
                 a reviewer"
                    .to_owned(),
            );
        }

        let mut sorted = self.assumptions.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() != self.assumptions.len() {
            problems.push("provenance.assumptions lists the same id twice".to_owned());
        }

        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance(review: ReviewStatus, reviewed_by: Option<&str>) -> Provenance {
        Provenance {
            sources: vec!["https://wiki.guildwars.com/wiki/Profession".to_owned()],
            crawled: "2026-09-22".parse().unwrap(),
            review,
            reviewed_by: reviewed_by.map(str::to_owned),
            assumptions: Vec::new(),
            notes: String::new(),
        }
    }

    #[test]
    fn a_complete_block_has_no_problems() {
        let block = provenance(ReviewStatus::Reviewed, Some("owner"));
        assert_eq!(block.problems(), Vec::<String>::new());
    }

    #[test]
    fn reviewed_without_a_reviewer_is_a_problem() {
        let block = provenance(ReviewStatus::Reviewed, None);
        assert_eq!(block.problems().len(), 1);
        assert!(block.problems()[0].contains("reviewed_by"));
    }

    #[test]
    fn draft_without_a_reviewer_is_fine() {
        assert!(provenance(ReviewStatus::Draft, None).problems().is_empty());
    }

    #[test]
    fn sources_may_not_be_empty() {
        let mut block = provenance(ReviewStatus::Draft, None);
        block.sources.clear();
        assert_eq!(block.problems().len(), 1);
    }

    #[test]
    fn duplicate_assumptions_are_a_problem() {
        let mut block = provenance(ReviewStatus::Draft, None);
        let id: AssumptionId = "A-018".parse().unwrap();
        block.assumptions = vec![id, id];
        assert_eq!(block.problems().len(), 1);
    }

    #[test]
    fn review_statuses_order_by_how_far_the_work_has_gone() {
        assert!(ReviewStatus::NumbersOnly < ReviewStatus::Draft);
        assert!(ReviewStatus::Draft < ReviewStatus::Reviewed);
    }
}
