//! The assumptions register (§8.8, §21).
//!
//! Every value the wiki does not document, or contradicts itself about, lives
//! here with an id. Code and data reference the id; neither writes the number
//! (ENG-4). A run reports which assumptions it relied on, so a result that
//! depends on a guess says so.
//!
//! WP1.6 adds typed keys and use-recording on top of this. The schema lands
//! here because T1.2.7 has to route `assumptions.ron` and T1.2.8 has to check
//! that every id a data file cites exists.

use serde::{Deserialize, Serialize};

use crate::ids::AssumptionId;
use crate::provenance::Provenance;

/// `data/assumptions.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssumptionsFile {
    pub provenance: Provenance,
    pub assumptions: Vec<Assumption>,
}

/// One assumption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assumption {
    pub id: AssumptionId,
    /// What is being assumed, in one sentence.
    pub statement: String,
    pub value: AssumptionValue,
    #[serde(default)]
    pub unit: Option<Unit>,
    /// Why this is an assumption rather than a fact.
    pub rationale: String,
    pub status: AssumptionStatus,
    #[serde(default)]
    pub sources: Vec<String>,
    /// Which task is expected to settle it, while it is still `Pending`.
    #[serde(default)]
    pub notes: String,
}

impl Assumption {
    /// Whether a value has been settled.
    pub fn is_pending(&self) -> bool {
        matches!(self.value, AssumptionValue::Pending)
    }
}

/// What an assumption's value is.
///
/// [`AssumptionValue::Pending`] is a first-class variant rather than an absent
/// field, so that "nobody has worked this out yet" survives into the type
/// system and cannot be mistaken for a measured zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssumptionValue {
    Number(f64),
    Millis(u32),
    Gwinches(f32),
    Percent(f32),
    /// A lookup table, such as a level mapping.
    Table(Vec<(String, f64)>),
    /// A rule that is not a number.
    Text(String),
    /// Not settled. A run that reads one of these is an error.
    Pending,
}

impl AssumptionValue {
    /// The name of this variant, for error messages about type mismatches.
    pub fn type_name(&self) -> &'static str {
        match self {
            AssumptionValue::Number(_) => "Number",
            AssumptionValue::Millis(_) => "Millis",
            AssumptionValue::Gwinches(_) => "Gwinches",
            AssumptionValue::Percent(_) => "Percent",
            AssumptionValue::Table(_) => "Table",
            AssumptionValue::Text(_) => "Text",
            AssumptionValue::Pending => "Pending",
        }
    }
}

/// What a value is measured in, where that is not implied by its variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unit {
    Seconds,
    Milliseconds,
    Gwinches,
    Percent,
    Pips,
    Health,
    Energy,
    Levels,
}

/// How much weight an assumption's value carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AssumptionStatus {
    /// A reasoned guess.
    Assumed,
    /// Measured in-game, or derived from something measured.
    Measured,
    /// Documented by the wiki after all, or confirmed by a developer.
    Confirmed,
}

/// The register, indexed for lookup.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Assumptions {
    entries: Vec<Assumption>,
}

impl Assumptions {
    /// Builds a register from a file's entries, which must already be sorted
    /// and free of duplicates — the loader checks both.
    pub fn new(entries: Vec<Assumption>) -> Self {
        Assumptions { entries }
    }

    /// One assumption, by id.
    pub fn get(&self, id: AssumptionId) -> Option<&Assumption> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Whether an id is in the register.
    pub fn contains(&self, id: AssumptionId) -> bool {
        self.get(id).is_some()
    }

    /// Every assumption, in id order.
    pub fn all(&self) -> &[Assumption] {
        &self.entries
    }

    /// How many there are.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the register is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The assumptions still waiting for a value.
    pub fn pending(&self) -> impl Iterator<Item = &Assumption> {
        self.entries.iter().filter(|entry| entry.is_pending())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assumption(id: &str, value: AssumptionValue) -> Assumption {
        Assumption {
            id: id.parse().unwrap(),
            statement: "something".to_owned(),
            value,
            unit: None,
            rationale: "because".to_owned(),
            status: AssumptionStatus::Assumed,
            sources: Vec::new(),
            notes: String::new(),
        }
    }

    #[test]
    fn a_register_finds_entries_by_id() {
        let register = Assumptions::new(vec![
            assumption("A-012", AssumptionValue::Millis(250)),
            assumption("A-030", AssumptionValue::Millis(180_000)),
        ]);
        assert!(register.contains("A-012".parse().unwrap()));
        assert!(!register.contains("A-999".parse().unwrap()));
        assert_eq!(register.len(), 2);
    }

    #[test]
    fn pending_is_a_value_not_an_absence() {
        // The distinction that matters: a Pending assumption is present in the
        // register and can be reported on. An absent one is a dangling
        // reference, which is a different problem.
        let register = Assumptions::new(vec![
            assumption("A-032", AssumptionValue::Pending),
            assumption("A-012", AssumptionValue::Millis(250)),
        ]);
        assert!(register.contains("A-032".parse().unwrap()));
        assert_eq!(register.pending().count(), 1);
        assert_eq!(register.pending().next().unwrap().id.to_string(), "A-032");
    }

    #[test]
    fn an_entry_round_trips() {
        let text = r#"(
            id: "A-030",
            statement: "A fight that runs this long counts as a loss.",
            value: Millis(180000),
            unit: Some(Milliseconds),
            rationale: "A modelling choice, not a game rule.",
            status: Assumed,
        )"#;
        let entry: Assumption = ron::from_str(text).expect("should parse");
        assert_eq!(entry.value, AssumptionValue::Millis(180_000));
        assert!(!entry.is_pending());

        let back: Assumption =
            ron::from_str(&ron::to_string(&entry).unwrap()).expect("should parse again");
        assert_eq!(back, entry);
    }

    #[test]
    fn value_types_are_named_for_error_messages() {
        assert_eq!(AssumptionValue::Pending.type_name(), "Pending");
        assert_eq!(AssumptionValue::Millis(1).type_name(), "Millis");
        assert_eq!(AssumptionValue::Number(1.0).type_name(), "Number");
    }

    #[test]
    fn a_table_valued_assumption_parses() {
        let entry: Assumption = ron::from_str(
            r#"(
                id: "A-005",
                statement: "Missing hard-mode attribute ranks.",
                value: Table([("bonus", 5.0), ("cap", 20.0)]),
                rationale: "The wiki gives hard-mode ranks for few foes.",
                status: Assumed,
            )"#,
        )
        .expect("should parse");
        assert_eq!(
            entry.value,
            AssumptionValue::Table(vec![("bonus".to_owned(), 5.0), ("cap".to_owned(), 20.0)])
        );
    }
}
