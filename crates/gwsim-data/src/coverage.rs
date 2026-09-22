//! What the data covers, and what it does not yet (§8.7, §21).

use std::collections::BTreeMap;

use crate::core::{Campaign, Profession};
use crate::dataset::DataSet;
use crate::foe::SkillRef;
use crate::ids::{AssumptionId, SkillId, Slug};
use crate::provenance::ReviewStatus;

/// The set of assumptions a run relied on.
///
/// Sorted and free of duplicates, so two runs that touched the same
/// assumptions report the same list (§10.13).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssumptionsUsed {
    ids: Vec<AssumptionId>,
}

impl AssumptionsUsed {
    /// Nothing recorded yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that something read an assumption.
    pub fn record(&mut self, id: AssumptionId) {
        if let Err(position) = self.ids.binary_search(&id) {
            self.ids.insert(position, id);
        }
    }

    /// Records several at once.
    pub fn record_all(&mut self, ids: impl IntoIterator<Item = AssumptionId>) {
        for id in ids {
            self.record(id);
        }
    }

    /// The ids, in order.
    pub fn ids(&self) -> &[AssumptionId] {
        &self.ids
    }

    /// Whether an assumption was read.
    pub fn contains(&self, id: AssumptionId) -> bool {
        self.ids.binary_search(&id).is_ok()
    }

    /// How many were read.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether nothing was read.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Merges another set into this one.
    pub fn merge(&mut self, other: &AssumptionsUsed) {
        self.record_all(other.ids.iter().copied());
    }
}

/// How much of the game the data covers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Coverage {
    /// Skills by profession and review status. `None` is the common pool.
    pub by_profession: BTreeMap<Option<Profession>, StatusCounts>,
    /// Skills by campaign.
    pub by_campaign: BTreeMap<Campaign, StatusCounts>,
    /// Totals.
    pub total: StatusCounts,
    /// Skills a foe uses that have no file at all.
    pub missing_skills: BTreeMap<Slug, Vec<Slug>>,
    /// Foes whose bars contain skills nobody has encoded.
    pub foes_with_unencoded_skills: BTreeMap<Slug, Vec<Slug>>,
    /// Skill ids a benchmark's template codes name that have no file yet.
    ///
    /// A benchmark is a frozen copy of someone else's build, so it routinely
    /// names skills gwsim has not reached. That makes this a coverage figure
    /// rather than a fault: it answers "how far am I from being able to
    /// reproduce this build", which is the question a benchmark exists to ask.
    pub benchmarks_missing_skills: BTreeMap<Slug, Vec<SkillId>>,
    /// Whether percentages are out of the whole game or only what exists.
    ///
    /// Until `data/skills/index.ron` arrives in T2.3.6 there is no denominator
    /// for "every skill in the game", so a percentage would be of the files
    /// that happen to exist — which is always 100%. Saying so is better than
    /// printing a meaningless number.
    pub denominator_is_complete: bool,
}

/// How many skills sit at each review status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusCounts {
    pub numbers_only: usize,
    pub draft: usize,
    pub reviewed: usize,
}

impl StatusCounts {
    /// Every skill counted.
    pub fn total(&self) -> usize {
        self.numbers_only + self.draft + self.reviewed
    }

    /// The share that has been encoded at all, as a percentage.
    pub fn encoded_percent(&self) -> f64 {
        if self.total() == 0 {
            return 0.0;
        }
        (self.draft + self.reviewed) as f64 * 100.0 / self.total() as f64
    }

    /// The share that has been reviewed, as a percentage.
    pub fn reviewed_percent(&self) -> f64 {
        if self.total() == 0 {
            return 0.0;
        }
        self.reviewed as f64 * 100.0 / self.total() as f64
    }

    fn count(&mut self, status: ReviewStatus) {
        match status {
            ReviewStatus::NumbersOnly => self.numbers_only += 1,
            ReviewStatus::Draft => self.draft += 1,
            ReviewStatus::Reviewed => self.reviewed += 1,
        }
    }
}

impl Coverage {
    /// Works out what a data set covers.
    pub fn compute(data: &DataSet) -> Coverage {
        let mut coverage = Coverage {
            // T2.3.6 adds data/skills/index.ron, which is the first real
            // denominator. Until then, every percentage is of what exists.
            denominator_is_complete: false,
            ..Coverage::default()
        };

        for entry in data.skills.values() {
            let skill = &entry.value;
            let status = skill.provenance.review;

            coverage
                .by_profession
                .entry(skill.profession)
                .or_default()
                .count(status);
            coverage
                .by_campaign
                .entry(skill.campaign)
                .or_default()
                .count(status);
            coverage.total.count(status);
        }

        for entry in data.foes.values() {
            let foe_slug = crate::ids::slugify(&entry.value.name);
            let mut missing = Vec::new();
            let mut unencoded = Vec::new();

            for foe_skill in &entry.value.skills {
                let SkillRef::Slug(slug) = &foe_skill.skill else {
                    continue;
                };
                match data.skill(slug) {
                    None => missing.push(slug.clone()),
                    Some(skill) if skill.provenance.review == ReviewStatus::NumbersOnly => {
                        unencoded.push(slug.clone())
                    }
                    Some(_) => {}
                }
            }

            if !missing.is_empty() {
                missing.sort();
                coverage.missing_skills.insert(foe_slug.clone(), missing);
            }
            if !unencoded.is_empty() {
                unencoded.sort();
                coverage
                    .foes_with_unencoded_skills
                    .insert(foe_slug, unencoded);
            }
        }

        for entry in data.benchmarks.values() {
            let slug = crate::ids::slugify(&entry.value.name);
            let mut missing: Vec<SkillId> = Vec::new();

            for slot in &entry.value.slots {
                // A code that does not decode is `validate`'s problem, not
                // this one, so it is passed over rather than reported twice.
                let Ok(template) = crate::template::SkillTemplate::decode(&slot.skill_code) else {
                    continue;
                };
                for id in template.skills.iter().flatten() {
                    if data.skill_by_id(*id).is_none() {
                        missing.push(*id);
                    }
                }
            }

            missing.sort();
            missing.dedup();
            if !missing.is_empty() {
                coverage.benchmarks_missing_skills.insert(slug, missing);
            }
        }

        coverage
    }

    /// Whether anything at all has been encoded.
    pub fn is_empty(&self) -> bool {
        self.total.total() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_assumptions_are_sorted_and_deduplicated() {
        let mut used = AssumptionsUsed::new();
        used.record("A-030".parse().unwrap());
        used.record("A-005".parse().unwrap());
        used.record("A-030".parse().unwrap());

        let shown: Vec<String> = used.ids().iter().map(|id| id.to_string()).collect();
        assert_eq!(shown, ["A-005", "A-030"]);
        assert_eq!(used.len(), 2);
    }

    #[test]
    fn the_recorded_set_is_exactly_what_was_read() {
        let mut used = AssumptionsUsed::new();
        used.record("A-012".parse().unwrap());
        assert!(used.contains("A-012".parse().unwrap()));
        assert!(!used.contains("A-013".parse().unwrap()));
    }

    #[test]
    fn merging_keeps_the_set_sorted() {
        let mut left = AssumptionsUsed::new();
        left.record_all(["A-030".parse().unwrap(), "A-001".parse().unwrap()]);
        let mut right = AssumptionsUsed::new();
        right.record_all(["A-012".parse().unwrap(), "A-001".parse().unwrap()]);

        left.merge(&right);
        let shown: Vec<String> = left.ids().iter().map(|id| id.to_string()).collect();
        assert_eq!(shown, ["A-001", "A-012", "A-030"]);
    }

    #[test]
    fn status_counts_add_up() {
        let counts = StatusCounts {
            numbers_only: 3,
            draft: 5,
            reviewed: 2,
        };
        assert_eq!(counts.total(), 10);
        assert_eq!(counts.encoded_percent(), 70.0);
        assert_eq!(counts.reviewed_percent(), 20.0);
    }

    #[test]
    fn an_empty_set_reports_zero_rather_than_dividing_by_zero() {
        let counts = StatusCounts::default();
        assert_eq!(counts.total(), 0);
        assert_eq!(counts.encoded_percent(), 0.0);
        assert_eq!(counts.reviewed_percent(), 0.0);
    }
}
