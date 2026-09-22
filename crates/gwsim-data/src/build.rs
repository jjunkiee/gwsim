//! A character's build, and the rules for whether it is legal (§7.2).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::core::{ArmorSlot, Attribute, Profession};
use crate::dataset::DataSet;
use crate::derived::{MAX_POINTS, MAX_POINTS_RANK, attribute_cost};
use crate::ids::{SkillId, Slug};
use crate::items::RuneKind;

/// How many skills a bar holds.
pub const BAR_SIZE: usize = 8;
/// How many elite skills a bar may carry.
pub const MAX_ELITES: usize = 1;
/// How many PvE-only skills a player's bar may carry.
pub const MAX_PVE_ONLY: usize = 3;

/// Who is using a build.
///
/// This is not cosmetic: heroes may not carry PvE-only skills at all, because
/// PvE-only skills are never unlocked for the account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlotKind {
    /// The player.
    Human,
    Hero,
    Henchman,
}

/// A character's skills, attributes and gear.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    pub primary: Profession,
    #[serde(default)]
    pub secondary: Option<Profession>,

    /// Attribute points spent, by attribute. Ranks, not raw points.
    #[serde(default)]
    pub attribute_points: BTreeMap<Attribute, u8>,

    /// The attribute headgear adds +1 to. Primary-profession only.
    #[serde(default)]
    pub headgear_attribute: Option<Attribute>,

    #[serde(default = "empty_bar")]
    pub skills: [Option<SkillId>; BAR_SIZE],

    #[serde(default = "bare_armor")]
    pub armor: [ArmorPiece; 5],

    #[serde(default)]
    pub weapon_set: WeaponSet,
}

fn empty_bar() -> [Option<SkillId>; BAR_SIZE] {
    [None; BAR_SIZE]
}

fn bare_armor() -> [ArmorPiece; 5] {
    ArmorSlot::ALL.map(|slot| ArmorPiece {
        slot,
        insignia: None,
        rune: None,
    })
}

/// One piece of armor, and what is fitted to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmorPiece {
    pub slot: ArmorSlot,
    #[serde(default)]
    pub insignia: Option<Slug>,
    #[serde(default)]
    pub rune: Option<Slug>,
}

/// The weapons a build holds.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponSet {
    #[serde(default)]
    pub main: Option<Slug>,
    #[serde(default)]
    pub offhand: Option<Slug>,
    #[serde(default)]
    pub prefix: Option<Slug>,
    #[serde(default)]
    pub suffix: Option<Slug>,
    #[serde(default)]
    pub inscription: Option<Slug>,
    #[serde(default)]
    pub offhand_upgrades: Vec<Slug>,
}

impl Build {
    /// A build with nothing chosen but a profession.
    pub fn new(primary: Profession) -> Self {
        Build {
            primary,
            secondary: None,
            attribute_points: BTreeMap::new(),
            headgear_attribute: None,
            skills: empty_bar(),
            armor: bare_armor(),
            weapon_set: WeaponSet::default(),
        }
    }

    /// Whether an attribute belongs to one of this build's professions.
    pub fn has_attribute(&self, attribute: Attribute) -> bool {
        let profession = attribute.profession();
        if profession == self.primary {
            return true;
        }
        match self.secondary {
            // A secondary profession grants its attributes *except* its
            // primary attribute, which only its own primaries get.
            Some(secondary) => profession == secondary && !attribute.is_primary(),
            None => false,
        }
    }

    /// Whether this build could take a rune for an attribute.
    ///
    /// Runes fit only the primary profession's armor, so a
    /// secondary-profession attribute can never exceed rank 12.
    pub fn can_rune(&self, attribute: Attribute) -> bool {
        attribute.profession() == self.primary
    }

    /// Every problem with this build, not just the first.
    ///
    /// The optimiser rejects thousands of candidates a second, so a caller
    /// that only needs a yes or no should use [`Build::is_legal`].
    pub fn check(&self, data: &DataSet, slot: SlotKind) -> Vec<LegalityError> {
        let mut problems = Vec::new();

        if Some(self.primary) == self.secondary {
            problems.push(LegalityError::SecondaryMatchesPrimary(self.primary));
        }

        self.check_attributes(&mut problems);
        self.check_skills(data, slot, &mut problems);
        self.check_gear(data, &mut problems);

        problems
    }

    /// Whether the build breaks no rule.
    pub fn is_legal(&self, data: &DataSet, slot: SlotKind) -> bool {
        self.check(data, slot).is_empty()
    }

    fn check_attributes(&self, problems: &mut Vec<LegalityError>) {
        let mut spent = 0u16;
        for (attribute, rank) in &self.attribute_points {
            if *rank > MAX_POINTS_RANK {
                problems.push(LegalityError::RankTooHigh {
                    attribute: *attribute,
                    rank: *rank,
                });
                continue;
            }
            if !self.has_attribute(*attribute) {
                problems.push(LegalityError::AttributeNotAvailable {
                    attribute: *attribute,
                    profession: attribute.profession(),
                });
            }
            spent += attribute_cost(*rank);
        }

        if spent > MAX_POINTS {
            problems.push(LegalityError::TooManyPoints { spent });
        }

        if let Some(attribute) = self.headgear_attribute
            && attribute.profession() != self.primary
        {
            problems.push(LegalityError::HeadgearNotPrimary {
                attribute,
                primary: self.primary,
            });
        }
    }

    fn check_skills(&self, data: &DataSet, slot: SlotKind, problems: &mut Vec<LegalityError>) {
        let mut seen: Vec<SkillId> = Vec::new();
        let mut elites = 0usize;
        let mut pve_only = 0usize;

        for id in self.skills.iter().flatten() {
            if seen.contains(id) {
                problems.push(LegalityError::DuplicateSkill(*id));
            } else {
                seen.push(*id);
            }

            // A skill we have no file for cannot be judged. That is a data
            // problem rather than a build problem, and `gwsim data validate`
            // is where it belongs, so it is reported once and skipped.
            let Some(skill) = data.skill_by_id(*id) else {
                problems.push(LegalityError::UnknownSkill(*id));
                continue;
            };

            if skill.elite {
                elites += 1;
            }
            if skill.pve_only {
                pve_only += 1;
            }

            // A skill with no profession is common and always allowed.
            if let Some(profession) = skill.profession {
                let allowed = profession == self.primary || Some(profession) == self.secondary;
                if !allowed && !skill.pve_only {
                    problems.push(LegalityError::SkillNotAvailable {
                        id: *id,
                        name: skill.name.clone(),
                        profession,
                    });
                }
            }
        }

        if elites > MAX_ELITES {
            problems.push(LegalityError::TooManyElites(elites));
        }

        match slot {
            // PvE-only skills are never unlocked for the account, so a hero
            // cannot carry one at all.
            SlotKind::Hero | SlotKind::Henchman if pve_only > 0 => {
                problems.push(LegalityError::PveOnlyOnHero(pve_only));
            }
            _ if pve_only > MAX_PVE_ONLY => {
                problems.push(LegalityError::TooManyPveOnly(pve_only));
            }
            _ => {}
        }
    }

    fn check_gear(&self, data: &DataSet, problems: &mut Vec<LegalityError>) {
        for piece in &self.armor {
            if let Some(slug) = &piece.rune {
                let Some(rune) = find_rune(data, slug) else {
                    problems.push(LegalityError::UnknownRune(slug.clone()));
                    continue;
                };
                if let RuneKind::Attribute { attribute, .. } = &rune.kind
                    && !self.can_rune(*attribute)
                {
                    problems.push(LegalityError::RuneNotPrimary {
                        rune: slug.clone(),
                        attribute: *attribute,
                        primary: self.primary,
                    });
                }
            }

            if let Some(slug) = &piece.insignia {
                let Some(insignia) = find_insignia(data, slug) else {
                    problems.push(LegalityError::UnknownInsignia(slug.clone()));
                    continue;
                };
                if let Some(profession) = insignia.profession
                    && profession != self.primary
                {
                    problems.push(LegalityError::InsigniaNotPrimary {
                        insignia: slug.clone(),
                        profession,
                        primary: self.primary,
                    });
                }
            }
        }
    }

    /// The armor piece in a slot.
    pub fn piece(&self, slot: ArmorSlot) -> &ArmorPiece {
        &self.armor[slot.index()]
    }
}

fn find_rune<'a>(data: &'a DataSet, slug: &Slug) -> Option<&'a crate::items::Rune> {
    data.runes
        .as_ref()?
        .value
        .runes
        .iter()
        .find(|rune| rune.slug == *slug)
}

fn find_insignia<'a>(data: &'a DataSet, slug: &Slug) -> Option<&'a crate::items::Insignia> {
    data.insignias
        .as_ref()?
        .value
        .insignias
        .iter()
        .find(|insignia| insignia.slug == *slug)
}

/// A rule a build breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalityError {
    SecondaryMatchesPrimary(Profession),
    AttributeNotAvailable {
        attribute: Attribute,
        profession: Profession,
    },
    RankTooHigh {
        attribute: Attribute,
        rank: u8,
    },
    TooManyPoints {
        spent: u16,
    },
    HeadgearNotPrimary {
        attribute: Attribute,
        primary: Profession,
    },
    SkillNotAvailable {
        id: SkillId,
        name: String,
        profession: Profession,
    },
    UnknownSkill(SkillId),
    DuplicateSkill(SkillId),
    TooManyElites(usize),
    TooManyPveOnly(usize),
    PveOnlyOnHero(usize),
    UnknownRune(Slug),
    RuneNotPrimary {
        rune: Slug,
        attribute: Attribute,
        primary: Profession,
    },
    UnknownInsignia(Slug),
    InsigniaNotPrimary {
        insignia: Slug,
        profession: Profession,
        primary: Profession,
    },
}

impl fmt::Display for LegalityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LegalityError::SecondaryMatchesPrimary(profession) => write!(
                f,
                "the secondary profession is also {profession:?}; a build needs two \
                 different professions, or none at all"
            ),
            LegalityError::AttributeNotAvailable {
                attribute,
                profession,
            } => write!(
                f,
                "{attribute:?} belongs to {profession:?}, which this build does not \
                 have (a secondary profession does not grant its primary attribute)"
            ),
            LegalityError::RankTooHigh { attribute, rank } => write!(
                f,
                "{attribute:?} is at rank {rank}; attribute points can only reach \
                 {MAX_POINTS_RANK}, and runes and headgear are counted separately"
            ),
            LegalityError::TooManyPoints { spent } => write!(
                f,
                "this build spends {spent} attribute points, but only {MAX_POINTS} are \
                 available"
            ),
            LegalityError::HeadgearNotPrimary { attribute, primary } => write!(
                f,
                "headgear adds +1 to {attribute:?}, which is not a {primary:?} \
                 attribute; headgear only boosts the primary profession"
            ),
            LegalityError::SkillNotAvailable {
                id,
                name,
                profession,
            } => write!(
                f,
                "{name} ({id}) is a {profession:?} skill, which this build cannot use"
            ),
            LegalityError::UnknownSkill(id) => {
                write!(f, "there is no skill file for {id}")
            }
            LegalityError::DuplicateSkill(id) => {
                write!(f, "{id} appears on the bar more than once")
            }
            LegalityError::TooManyElites(count) => write!(
                f,
                "this bar carries {count} elite skills; only {MAX_ELITES} is allowed"
            ),
            LegalityError::TooManyPveOnly(count) => write!(
                f,
                "this bar carries {count} PvE-only skills; at most {MAX_PVE_ONLY} are \
                 allowed"
            ),
            LegalityError::PveOnlyOnHero(count) => write!(
                f,
                "this bar carries {count} PvE-only skills, and heroes cannot use them \
                 at all: PvE-only skills are never unlocked for the account"
            ),
            LegalityError::UnknownRune(slug) => {
                write!(f, "there is no rune called {slug}")
            }
            LegalityError::RuneNotPrimary {
                rune,
                attribute,
                primary,
            } => write!(
                f,
                "{rune} raises {attribute:?}, but runes only fit the primary \
                 profession's armor and this build is {primary:?}"
            ),
            LegalityError::UnknownInsignia(slug) => {
                write!(f, "there is no insignia called {slug}")
            }
            LegalityError::InsigniaNotPrimary {
                insignia,
                profession,
                primary,
            } => write!(
                f,
                "{insignia} is {profession:?} armor, and this build is {primary:?}"
            ),
        }
    }
}

impl std::error::Error for LegalityError {}

// ------------------------------------------------------- template conversion

use crate::template::{EquipmentTemplate, SkillTemplate};

/// A problem turning a template code into a build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FromTemplateError {
    /// Skill ids in the code that no data file claims.
    UnknownSkills(Vec<SkillId>),
}

impl fmt::Display for FromTemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FromTemplateError::UnknownSkills(ids) => {
                let list: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
                write!(
                    f,
                    "this code uses {} skill(s) with no data file: {}",
                    ids.len(),
                    list.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for FromTemplateError {}

/// Things worth telling the caller that are not errors.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplateWarnings {
    /// Equipment item ids that map to no known item.
    pub unknown_items: Vec<u32>,
    /// Equipment modifier ids that map to no known rune or insignia.
    pub unknown_modifiers: Vec<u32>,
}

impl Build {
    /// Builds a character from a template code.
    ///
    /// **Runes are left empty when there is no equipment template**, because a
    /// skill template does not carry them. That is not an oversight in the
    /// conversion: the attribute ranks in a skill code are from points alone,
    /// so a build produced from one is genuinely rune-less until gear is
    /// added.
    pub fn from_templates(
        skill: &SkillTemplate,
        equipment: Option<&EquipmentTemplate>,
        data: &DataSet,
    ) -> Result<(Build, TemplateWarnings), FromTemplateError> {
        let mut build = Build::new(skill.primary);
        build.secondary = skill.secondary;
        build.skills = skill.skills;

        for (attribute, rank) in &skill.attributes {
            if *rank > 0 {
                build.attribute_points.insert(*attribute, *rank);
            }
        }

        let unknown: Vec<SkillId> = skill
            .skills
            .iter()
            .flatten()
            .filter(|id| data.skill_by_id(**id).is_none())
            .copied()
            .collect();
        if !unknown.is_empty() {
            return Err(FromTemplateError::UnknownSkills(unknown));
        }

        let mut warnings = TemplateWarnings::default();
        if let Some(equipment) = equipment {
            build.apply_equipment(equipment, data, &mut warnings);
        }

        Ok((build, warnings))
    }

    fn apply_equipment(
        &mut self,
        equipment: &EquipmentTemplate,
        data: &DataSet,
        warnings: &mut TemplateWarnings,
    ) {
        for item in &equipment.items {
            let Some(slot) = armor_slot_of(item.slot) else {
                continue;
            };
            for modifier in &item.modifiers {
                if let Some(rune) = rune_with_modifier_id(data, *modifier) {
                    self.armor[slot.index()].rune = Some(rune.clone());
                } else if let Some(insignia) = insignia_with_modifier_id(data, *modifier) {
                    self.armor[slot.index()].insignia = Some(insignia.clone());
                } else {
                    warnings.unknown_modifiers.push(*modifier);
                }
            }
            warnings.unknown_items.push(item.item_id);
        }
        warnings.unknown_items.sort_unstable();
        warnings.unknown_items.dedup();
        warnings.unknown_modifiers.sort_unstable();
        warnings.unknown_modifiers.dedup();
    }

    /// Writes this build out as template codes.
    ///
    /// The skill code is faithful. **The equipment code is not**, and cannot
    /// be: equipment templates describe PvP items, and gwsim builds are PvE
    /// (T1.3.1 §4). It carries the runes and insignias whose template ids are
    /// known, and nothing else.
    pub fn to_templates(&self, data: &DataSet) -> (SkillTemplate, EquipmentTemplate) {
        let attributes: Vec<(Attribute, u8)> = self
            .attribute_points
            .iter()
            .map(|(attribute, rank)| (*attribute, *rank))
            .collect();

        let skill = SkillTemplate {
            primary: self.primary,
            secondary: self.secondary,
            attributes,
            skills: self.skills,
        };

        let mut items = Vec::new();
        for piece in &self.armor {
            let mut modifiers = Vec::new();
            if let Some(slug) = &piece.insignia
                && let Some(id) = find_insignia(data, slug).and_then(|i| i.template_modifier_id)
            {
                modifiers.push(id);
            }
            if let Some(slug) = &piece.rune
                && let Some(id) = find_rune(data, slug).and_then(|r| r.template_modifier_id)
            {
                modifiers.push(id);
            }
            if modifiers.is_empty() {
                continue;
            }
            items.push(crate::template::EquipmentItem {
                slot: equipment_slot_of(piece.slot),
                item_id: 0,
                dye: crate::template::Dye(9),
                modifiers,
            });
        }

        (skill, EquipmentTemplate { items })
    }
}

/// The armor slot an equipment slot corresponds to, if it is armor at all.
fn armor_slot_of(slot: crate::template::EquipmentSlot) -> Option<ArmorSlot> {
    use crate::template::EquipmentSlot as E;
    Some(match slot {
        E::Head => ArmorSlot::Head,
        E::Chest => ArmorSlot::Chest,
        E::Hands => ArmorSlot::Hands,
        E::Legs => ArmorSlot::Legs,
        E::Feet => ArmorSlot::Feet,
        E::Weapon | E::OffHand => return None,
    })
}

fn equipment_slot_of(slot: ArmorSlot) -> crate::template::EquipmentSlot {
    use crate::template::EquipmentSlot as E;
    match slot {
        ArmorSlot::Head => E::Head,
        ArmorSlot::Chest => E::Chest,
        ArmorSlot::Hands => E::Hands,
        ArmorSlot::Legs => E::Legs,
        ArmorSlot::Feet => E::Feet,
    }
}

fn rune_with_modifier_id(data: &DataSet, id: u32) -> Option<&Slug> {
    data.runes
        .as_ref()?
        .value
        .runes
        .iter()
        .find(|rune| rune.template_modifier_id == Some(id))
        .map(|rune| &rune.slug)
}

fn insignia_with_modifier_id(data: &DataSet, id: u32) -> Option<&Slug> {
    data.insignias
        .as_ref()?
        .value
        .insignias
        .iter()
        .find(|insignia| insignia.template_modifier_id == Some(id))
        .map(|insignia| &insignia.slug)
}
