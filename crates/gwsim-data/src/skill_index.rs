//! `data/skills/index.ron`: every player skill in the game (T2.3.6).
//!
//! The index is facts only — an id, a title and a few flags per skill — and
//! exists for one reason: it is the denominator coverage needs. Without it,
//! "how much of the Mesmer is encoded" can only be answered as a share of the
//! files that happen to exist, which is always 100%.
//!
//! The extractor writes it from the wiki's list pages (WP2.3). Monster skills
//! are not in it: they are added when an encounter needs one (D6), so they
//! have no fixed denominator.

use serde::{Deserialize, Serialize};

use crate::core::{Campaign, Profession};
use crate::ids::{SkillId, Slug, WikiTitle};
use crate::provenance::Provenance;

/// The index file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillIndexFile {
    pub provenance: Provenance,
    /// One entry per player skill, in id order.
    pub skills: Vec<IndexedSkill>,
}

/// One player skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedSkill {
    pub id: SkillId,
    /// The wiki page title.
    pub title: WikiTitle,
    /// The file stem the skill's data file will have.
    pub slug: Slug,
    /// [`None`] for common skills and profession-less PvE-only skills.
    #[serde(default)]
    pub profession: Option<Profession>,
    #[serde(default)]
    pub elite: bool,
    #[serde(default)]
    pub pve_only: bool,
    /// [`None`] where the list page names a campaign gwsim has no variant
    /// for; the extractor reports those.
    #[serde(default)]
    pub campaign: Option<Campaign>,
}

impl SkillIndexFile {
    /// The entry for an id, if the index has one.
    pub fn by_id(&self, id: SkillId) -> Option<&IndexedSkill> {
        self.skills
            .binary_search_by_key(&id, |skill| skill.id)
            .ok()
            .map(|index| &self.skills[index])
    }

    /// Whether the entries are in strictly increasing id order, which
    /// [`Self::by_id`] relies on.
    pub fn is_sorted(&self) -> bool {
        self.skills.windows(2).all(|pair| pair[0].id < pair[1].id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = r#"(
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Skill_template_format/Skill_list"],
        crawled: "2026-09-23",
        review: Draft,
    ),
    skills: [
        (id: 39, title: "Energy Surge", slug: "energy-surge", profession: Some(Mesmer), elite: true, campaign: Some(Core)),
        (id: 2416, title: "Air of Superiority", slug: "air-of-superiority", pve_only: true, campaign: Some(EyeOfTheNorth)),
    ],
)"#;

    #[test]
    fn an_index_parses_and_looks_up_by_id() {
        let index: SkillIndexFile = ron::from_str(INDEX).expect("should parse");
        assert!(index.is_sorted());
        let skill = index.by_id(SkillId(39)).expect("should be found");
        assert_eq!(skill.title.as_str(), "Energy Surge");
        assert!(skill.elite);
        assert_eq!(index.by_id(SkillId(2416)).unwrap().profession, None);
        assert!(index.by_id(SkillId(40)).is_none());
    }
}
