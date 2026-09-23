//! Party files: who is in the party and what they carry (§15, T4.9.1).
//!
//! `data/parties/<name>.ron` holds one entry per slot: its kind, its build,
//! whether the optimiser may change it, and, for a human slot, the priority
//! plan it plays by. User parties use the same format in the user data
//! directory.

use serde::{Deserialize, Serialize};

use crate::build::{Build, SlotKind};
use crate::foe::SkillRef;
use crate::ids::Slug;
use crate::plan::PriorityPlan;

/// A party.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyFile {
    pub name: String,
    #[serde(default)]
    pub notes: String,
    /// Where the builds were read from, for benchmark parties.
    #[serde(default)]
    pub sources: Vec<String>,
    pub slots: Vec<PartySlot>,
    /// Overrides for the tactics plan, for every situation this party runs
    /// (§11.6). A situation's own overrides win over these.
    #[serde(default)]
    pub tactics: Option<crate::tactics::TacticsOverrides>,
}

/// One party member.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartySlot {
    /// A label for reports, such as `player` or `hero 7`.
    pub name: String,
    pub kind: SlotKind,
    /// The hero, for hero slots. Identity matters only for profession and
    /// availability (D17), so this is optional.
    #[serde(default)]
    pub hero: Option<Slug>,
    pub build: Build,
    /// The optimiser never changes a locked slot (OPT-3).
    #[serde(default)]
    pub locked: bool,
    /// A human slot's plan. Generated from the build when absent (WP4.6).
    #[serde(default)]
    pub plan: Option<PriorityPlan>,
    /// Skills this slot must never use automatically (AI-H9).
    #[serde(default)]
    pub disabled_skills: Vec<SkillRef>,
    /// The template code as published, for benchmark slots.
    #[serde(default)]
    pub published_code: Option<String>,
}
