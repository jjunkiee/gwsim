//! Account profiles: what an account actually has (§7.2, T5.7.3, OPT-2).
//!
//! A profile limits a search to the skills an account has unlocked, the
//! heroes it owns, the upgrades it can use and the title ranks it has
//! reached, which set PvE-only skill values. **With no profile, everything is
//! unlocked and every title is at its maximum** (Q14), which is also what
//! [`AccountProfile::default`] gives.
//!
//! Profiles are RON files in the user directory's `profiles/` folder
//! (`%APPDATA%\gwsim\profiles\<name>.ron` on Windows).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::core::TitleTrack;
use crate::ids::{SkillId, Slug};

/// Everything, or only these.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Unlocks<T: Ord> {
    #[default]
    All,
    Only(BTreeSet<T>),
}

impl<T: Ord> Unlocks<T> {
    /// Whether this item is available.
    pub fn allows(&self, item: &T) -> bool {
        match self {
            Unlocks::All => true,
            Unlocks::Only(set) => set.contains(item),
        }
    }
}

/// An account's unlocks and ranks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountProfile {
    /// A label for reports.
    #[serde(default)]
    pub name: String,
    /// Skills unlocked on the account, by template id.
    #[serde(default)]
    pub unlocked_skills: Unlocks<SkillId>,
    /// Title ranks. A track not listed is at its maximum.
    #[serde(default)]
    pub title_ranks: BTreeMap<TitleTrack, u8>,
    /// Heroes owned, by slug.
    #[serde(default)]
    pub heroes_owned: Unlocks<Slug>,
    /// Whether the account owns Eye of the North, whose skills and heroes
    /// need it.
    #[serde(default = "yes")]
    pub eotn_owned: bool,
    /// Runes, insignias and weapon upgrades available, by slug.
    #[serde(default)]
    pub available_upgrades: Unlocks<Slug>,
    /// Skills each character has learned, for Melandru's Accord, which
    /// limits a character to its own learned skills (§10.12).
    #[serde(default)]
    pub learned_skills: Option<BTreeMap<String, BTreeSet<SkillId>>>,
}

fn yes() -> bool {
    true
}

impl Default for AccountProfile {
    fn default() -> Self {
        AccountProfile {
            name: "default".to_owned(),
            unlocked_skills: Unlocks::All,
            title_ranks: BTreeMap::new(),
            heroes_owned: Unlocks::All,
            eotn_owned: true,
            available_upgrades: Unlocks::All,
            learned_skills: None,
        }
    }
}

impl AccountProfile {
    /// Whether a skill may be used by a character (or, with `None`, by the
    /// account as a whole). Under Melandru's Accord a character is limited to
    /// its learned skills, where the profile records them.
    pub fn allows_skill(&self, id: SkillId, character: Option<&str>, accord: bool) -> bool {
        if !self.unlocked_skills.allows(&id) {
            return false;
        }
        if accord
            && let (Some(learned), Some(character)) = (&self.learned_skills, character)
            && let Some(skills) = learned.get(character)
        {
            return skills.contains(&id);
        }
        true
    }

    /// The title ranks a fight should use: this profile's, or none at all
    /// under Melandru's Accord, which switches account titles off.
    pub fn effective_title_ranks(&self, accord: bool) -> BTreeMap<TitleTrack, u8> {
        if accord {
            TitleTrack::ALL.into_iter().map(|t| (t, 0)).collect()
        } else {
            self.title_ranks.clone()
        }
    }

    /// Reads a profile file.
    pub fn read(path: &Path) -> Result<AccountProfile, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        ron::from_str(&text).map_err(|e| format!("{} is not a profile: {e}", path.display()))
    }

    /// Writes a profile file, creating its folder.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
    }

    /// Where a named profile lives in a user directory.
    pub fn path_in(profiles: &Path, name: &str) -> PathBuf {
        profiles.join(format!("{name}.ron"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_profile_allows_everything() {
        let profile = AccountProfile::default();
        assert!(profile.allows_skill(SkillId(39), None, false));
        assert!(profile.heroes_owned.allows(&"gwen".parse().unwrap()));
        assert!(
            profile.title_ranks.is_empty(),
            "tracks default to their maximum"
        );
    }

    #[test]
    fn a_profile_round_trips_through_ron() {
        let mut profile = AccountProfile {
            name: "mine".to_owned(),
            unlocked_skills: Unlocks::Only([SkillId(39)].into()),
            ..AccountProfile::default()
        };
        profile.title_ranks.insert(TitleTrack::Asura, 3);
        let text = ron::ser::to_string_pretty(&profile, ron::ser::PrettyConfig::default()).unwrap();
        let back: AccountProfile = ron::from_str(&text).unwrap();
        assert_eq!(back, profile);
        assert!(!back.allows_skill(SkillId(75), None, false));
    }

    #[test]
    fn melandrus_accord_limits_a_character_to_its_learned_skills() {
        let learned: BTreeSet<SkillId> = [SkillId(39)].into();
        let profile = AccountProfile {
            learned_skills: Some([("Mira".to_owned(), learned)].into()),
            ..AccountProfile::default()
        };
        let surge = SkillId(39);
        let echo = SkillId(75);
        assert!(profile.allows_skill(echo, Some("Mira"), false));
        assert!(!profile.allows_skill(echo, Some("Mira"), true));
        assert!(profile.allows_skill(surge, Some("Mira"), true));
        assert!(
            profile
                .effective_title_ranks(true)
                .values()
                .all(|r| *r == 0)
        );
    }
}
