//! What each free slot may choose from (T5.1.2, T5.7.5, OPT-2, OPT-4).
//!
//! A slot's pools are computed once per search:
//!
//! - **skills**: every skill with an encoding (`Draft` or `Reviewed`; only
//!   `Reviewed` with `reviewed_only`, OPT-4) that the account has unlocked,
//!   of any profession the slot could take, plus common skills. PvE-only
//!   skills only for a human slot. Eye of the North skills need the
//!   expansion. Under Melandru's Accord a character keeps only its learned
//!   skills;
//! - **professions**: a human slot may take any primary; a hero's is fixed;
//! - **runes and insignias**: those the account has, filtered per primary
//!   profession when drawn.
//!
//! Which of the skills a particular build may carry depends on its
//! profession pair, so that filter is applied at draw time
//! ([`SlotPools::skills_for`]).

use gwsim_data::build::SlotKind;
use gwsim_data::core::{Campaign, Profession};
use gwsim_data::dataset::DataSet;
use gwsim_data::ids::{SkillId, Slug};
use gwsim_data::items::{Rune, RuneKind};
use gwsim_data::party::PartySlot;
use gwsim_data::profile::AccountProfile;
use gwsim_data::provenance::ReviewStatus;
use gwsim_data::skill::Skill;

/// Options that shape every pool.
#[derive(Debug, Clone, Default)]
pub struct PoolOptions {
    /// Only `Reviewed` skills (OPT-4).
    pub reviewed_only: bool,
    pub profile: AccountProfile,
    /// Melandru's Accord is on in a situation of the set.
    pub accord: bool,
}

/// The choices open to one free slot.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotPools {
    pub kind: SlotKind,
    /// The primaries it may take: all ten for a human slot, its own for a
    /// hero or henchman.
    pub primaries: Vec<Profession>,
    /// Every candidate skill, sorted by id.
    pub skills: Vec<SkillId>,
    /// Runes and insignias the account has, sorted by slug.
    pub runes: Vec<Slug>,
    pub insignias: Vec<Slug>,
}

/// Whether a skill can be simulated at all under these options (OPT-4).
pub fn usable(skill: &Skill, options: &PoolOptions) -> bool {
    skill.encoding.is_some()
        && match skill.provenance.review {
            ReviewStatus::Reviewed => true,
            ReviewStatus::Draft => !options.reviewed_only,
            ReviewStatus::NumbersOnly => false,
        }
}

impl SlotPools {
    /// The pools for a party slot.
    pub fn for_slot(slot: &PartySlot, data: &DataSet, options: &PoolOptions) -> SlotPools {
        let human = slot.kind == SlotKind::Human;
        let character = human.then_some(slot.name.as_str());
        let mut skills: Vec<SkillId> = data
            .skills
            .values()
            .map(|entry| &entry.value)
            .filter(|skill| usable(skill, options))
            .filter(|skill| human || !skill.pve_only)
            .filter(|skill| options.profile.eotn_owned || skill.campaign != Campaign::EyeOfTheNorth)
            .filter(|skill| {
                options
                    .profile
                    .allows_skill(skill.id, character, options.accord)
            })
            .map(|skill| skill.id)
            .collect();
        skills.sort();
        skills.dedup();

        let upgrades = &options.profile.available_upgrades;
        let mut runes: Vec<Slug> = data
            .runes
            .as_ref()
            .map(|file| {
                file.value
                    .runes
                    .iter()
                    .filter(|r| upgrades.allows(&r.slug))
                    .map(|r| r.slug.clone())
                    .collect()
            })
            .unwrap_or_default();
        runes.sort();
        let mut insignias: Vec<Slug> = data
            .insignias
            .as_ref()
            .map(|file| {
                file.value
                    .insignias
                    .iter()
                    .filter(|i| upgrades.allows(&i.slug))
                    .map(|i| i.slug.clone())
                    .collect()
            })
            .unwrap_or_default();
        insignias.sort();

        SlotPools {
            kind: slot.kind,
            primaries: if human {
                Profession::ALL.to_vec()
            } else {
                vec![slot.build.primary]
            },
            skills,
            runes,
            insignias,
        }
    }

    /// The candidate skills a build with this profession pair may carry.
    pub fn skills_for(
        &self,
        data: &DataSet,
        primary: Profession,
        secondary: Option<Profession>,
    ) -> Vec<SkillId> {
        self.skills
            .iter()
            .copied()
            .filter(|id| {
                data.skill_by_id(*id)
                    .is_some_and(|skill| match skill.profession {
                        None => true,
                        Some(p) => p == primary || Some(p) == secondary || skill.pve_only,
                    })
            })
            .collect()
    }

    /// The runes a primary profession's armor takes.
    pub fn runes_for<'a>(&self, data: &'a DataSet, primary: Profession) -> Vec<&'a Rune> {
        let Some(file) = data.runes.as_ref() else {
            return Vec::new();
        };
        file.value
            .runes
            .iter()
            .filter(|r| self.runes.contains(&r.slug))
            .filter(|r| match &r.kind {
                RuneKind::Attribute { attribute, .. } => attribute.profession() == primary,
                _ => true,
            })
            .collect()
    }

    /// The insignias a primary profession's armor takes.
    pub fn insignias_for(&self, data: &DataSet, primary: Profession) -> Vec<Slug> {
        let Some(file) = data.insignias.as_ref() else {
            return Vec::new();
        };
        file.value
            .insignias
            .iter()
            .filter(|i| self.insignias.contains(&i.slug))
            .filter(|i| i.profession.is_none() || i.profession == Some(primary))
            .map(|i| i.slug.clone())
            .collect()
    }
}
