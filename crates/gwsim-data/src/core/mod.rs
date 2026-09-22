//! The fixed vocabulary of the game.
//!
//! The split this module keeps: **identities live in code, numbers live in
//! data**. Which professions exist, which attribute belongs to which
//! profession and how the skill types nest are facts that cannot change
//! without a code change, so they are enums here. Base armor, energy, gwinches
//! and level formulas are values that can be corrected by editing
//! `data/core/*.ron`, so they are loaded, never hard-coded.

mod armor;
mod attribute;
mod campaign;
mod condition;
mod damage;
mod profession;
mod range;
mod skill_type;
mod tables;

pub use armor::{ArmorClassBonus, ArmorSlot, DamageClass};
pub use attribute::{Attribute, InherentEffect};
pub use campaign::{Campaign, TitleTrack};
pub use condition::Condition;
pub use damage::DamageType;
pub use profession::Profession;
pub use range::RangeBand;
pub use skill_type::SkillType;
pub use tables::{
    Assumed, AttributeRecord, AttributesFile, ConditionEffect, ConditionRecord, ConditionsFile,
    DamageMultiplier, DhuumsCovenantRules, HardModeRules, HmLevelKind, HmLevelRow, HmLevelTables,
    LevelsFile, MelandrusAccordRules, ModesFile, OptionalModeRules, ProfessionRecord,
    ProfessionsFile, RangesFile, ReforgedModeRules, TitleTrackRecord, TitlesFile,
};

use std::fmt;
use std::path::{Path, PathBuf};

/// Every `data/core/*.ron` file, loaded and checked.
///
/// The record collections are held in enum order and verified complete at
/// load, so the accessors below cannot fail and do not return [`Option`].
#[derive(Debug, Clone, PartialEq)]
pub struct CoreData {
    professions: ProfessionsFile,
    attributes: AttributesFile,
    conditions: ConditionsFile,
    ranges: RangesFile,
    titles: TitlesFile,
    /// Level formulas and the hard-mode level mappings.
    pub levels: LevelsFile,
    /// Game-mode rules.
    pub modes: ModesFile,
}

impl CoreData {
    /// The seven file names this loads, in load order.
    pub const FILES: [&'static str; 7] = [
        "professions.ron",
        "attributes.ron",
        "conditions.ron",
        "ranges.ron",
        "levels.ron",
        "modes.ron",
        "titles.ron",
    ];

    /// Reads and checks every core file in a directory.
    ///
    /// Every problem found is reported, not just the first, so one run tells
    /// an author everything they need to fix.
    pub fn load(dir: impl AsRef<Path>) -> Result<CoreData, LoadErrors> {
        let dir = dir.as_ref();
        let mut errors = Vec::new();

        let professions = read_file::<ProfessionsFile>(dir, "professions.ron", &mut errors);
        let attributes = read_file::<AttributesFile>(dir, "attributes.ron", &mut errors);
        let conditions = read_file::<ConditionsFile>(dir, "conditions.ron", &mut errors);
        let ranges = read_file::<RangesFile>(dir, "ranges.ron", &mut errors);
        let levels = read_file::<LevelsFile>(dir, "levels.ron", &mut errors);
        let modes = read_file::<ModesFile>(dir, "modes.ron", &mut errors);
        let titles = read_file::<TitlesFile>(dir, "titles.ron", &mut errors);

        let (Some(mut professions), Some(mut attributes), Some(mut conditions)) =
            (professions, attributes, conditions)
        else {
            return Err(LoadErrors(errors));
        };
        let (Some(mut ranges), Some(levels), Some(modes), Some(titles)) =
            (ranges, levels, modes, titles)
        else {
            return Err(LoadErrors(errors));
        };

        canonicalise(
            "professions.ron",
            "professions",
            &mut professions.professions,
            &Profession::ALL,
            |record| record.key,
            &mut errors,
        );
        canonicalise(
            "attributes.ron",
            "attributes",
            &mut attributes.attributes,
            &Attribute::ALL,
            |record| record.key,
            &mut errors,
        );
        canonicalise(
            "conditions.ron",
            "conditions",
            &mut conditions.conditions,
            &Condition::ALL,
            |record| record.key,
            &mut errors,
        );
        canonicalise(
            "ranges.ron",
            "bands",
            &mut ranges.bands,
            &RangeBand::ALL,
            |(band, _)| *band,
            &mut errors,
        );

        let data = CoreData {
            professions,
            attributes,
            conditions,
            ranges,
            titles,
            levels,
            modes,
        };
        data.check(&mut errors);

        if errors.is_empty() {
            Ok(data)
        } else {
            Err(LoadErrors(errors))
        }
    }

    /// The checks that need more than one file, or that compare data against
    /// the identity graph in code.
    fn check(&self, errors: &mut Vec<LoadError>) {
        for (file, provenance) in [
            ("professions.ron", &self.professions.provenance),
            ("attributes.ron", &self.attributes.provenance),
            ("conditions.ron", &self.conditions.provenance),
            ("ranges.ron", &self.ranges.provenance),
            ("levels.ron", &self.levels.provenance),
            ("modes.ron", &self.modes.provenance),
            ("titles.ron", &self.titles.provenance),
        ] {
            for problem in provenance.problems() {
                errors.push(LoadError::new(file, problem));
            }
        }

        // The primary flag in the data has to agree with the identity graph in
        // code. Either could hold the typo, so the message names both.
        for record in &self.attributes.attributes {
            let derived = record.key.is_primary();
            if record.primary != derived {
                errors.push(LoadError::new(
                    "attributes.ron",
                    format!(
                        "{:?} is marked primary: {} here but {} by Attribute::is_primary \
                         (its profession {:?} has primary {:?})",
                        record.key,
                        record.primary,
                        derived,
                        record.key.profession(),
                        record.key.profession().primary_attribute(),
                    ),
                ));
            }
            if record.primary && record.inherent.is_empty() {
                errors.push(LoadError::new(
                    "attributes.ron",
                    format!(
                        "{:?} is a primary attribute but has no inherent effect; every \
                         primary attribute has one",
                        record.key
                    ),
                ));
            }
            let mut sorted = record.inherent.clone();
            sorted.sort_unstable();
            sorted.dedup();
            if sorted.len() != record.inherent.len() {
                errors.push(LoadError::new(
                    "attributes.ron",
                    format!("{:?} lists the same inherent effect twice", record.key),
                ));
            }
        }

        // Ranges must grow with the band order, which is what lets callers
        // compare bands without looking up both numbers.
        for pair in self.ranges.bands.windows(2) {
            let [(before, near), (after, far)] = pair else {
                continue;
            };
            if far <= near {
                errors.push(LoadError::new(
                    "ranges.ron",
                    format!(
                        "{after:?} is {far} gwinches but {before:?} is {near}: bands must \
                         increase in the order RangeBand declares them"
                    ),
                ));
            }
        }

        for record in &self.conditions.conditions {
            if record.effects.is_empty() {
                errors.push(LoadError::new(
                    "conditions.ron",
                    format!("{:?} has no effects", record.key),
                ));
            }
        }

        // Burning is the only condition spirits can suffer.
        let spirit_conditions: Vec<Condition> = self
            .conditions
            .conditions
            .iter()
            .filter(|record| record.affects_spirits)
            .map(|record| record.key)
            .collect();
        if spirit_conditions != vec![Condition::Burning] {
            errors.push(LoadError::new(
                "conditions.ron",
                format!(
                    "affects_spirits is set on {spirit_conditions:?}, but Burning is the \
                     only condition spirits can suffer"
                ),
            ));
        }

        for track in &self.titles.tracks {
            let expected = usize::from(track.max_rank) + 1;
            if track.effective_ranks.len() != expected {
                errors.push(LoadError::new(
                    "titles.ron",
                    format!(
                        "{:?} goes to rank {} so needs {} effective ranks (rank 0 \
                         included), but has {}",
                        track.key,
                        track.max_rank,
                        expected,
                        track.effective_ranks.len()
                    ),
                ));
            }
            if track
                .effective_ranks
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            {
                errors.push(LoadError::new(
                    "titles.ron",
                    format!("{:?} has effective ranks that go down", track.key),
                ));
            }
        }

        for (name, kind) in [
            ("non_boss", HmLevelKind::NonBoss),
            ("boss", HmLevelKind::Boss),
            ("ally", HmLevelKind::Ally),
        ] {
            for row in self.levels.hm_levels.rows(kind) {
                if row.nm_min > row.nm_max || row.hm_min > row.hm_max {
                    errors.push(LoadError::new(
                        "levels.ron",
                        format!("hm_levels.{name} has a row whose range runs backwards: {row:?}"),
                    ));
                }
            }
        }
    }

    /// One profession's numbers.
    pub fn profession(&self, profession: Profession) -> &ProfessionRecord {
        &self.professions.professions[profession.index()]
    }

    /// One attribute's role.
    pub fn attribute(&self, attribute: Attribute) -> &AttributeRecord {
        &self.attributes.attributes[attribute.index()]
    }

    /// One condition's fixed effects.
    pub fn condition(&self, condition: Condition) -> &ConditionRecord {
        &self.conditions.conditions[condition.index()]
    }

    /// How far a named band reaches, in gwinches.
    pub fn gwinches(&self, band: RangeBand) -> f32 {
        self.ranges.bands[band.index()].1
    }

    /// One title track's table, if it has been modelled yet.
    pub fn title_track(&self, track: TitleTrack) -> Option<&TitleTrackRecord> {
        self.titles.tracks.iter().find(|record| record.key == track)
    }

    /// Maximum energy for a profession, before skills, weapons and runes.
    pub fn max_energy(&self, profession: Profession) -> u16 {
        self.professions.base_energy + self.profession(profession).armor_energy_total()
    }

    /// Energy regeneration in pips for a profession, before items.
    pub fn energy_regen_pips(&self, profession: Profession) -> u8 {
        self.professions.base_energy_regen_pips
            + self.profession(profession).armor_regen_pips_total()
    }

    /// Energy regeneration in pips for a *foe* of a profession, which is one
    /// pip more than a player gets.
    pub fn foe_energy_regen_pips(&self, profession: Profession) -> u8 {
        self.energy_regen_pips(profession) + self.professions.foe_extra_energy_regen_pips
    }

    /// The whole professions file, for callers that need its other fields.
    pub fn professions_file(&self) -> &ProfessionsFile {
        &self.professions
    }

    /// The whole attributes file.
    pub fn attributes_file(&self) -> &AttributesFile {
        &self.attributes
    }

    /// The whole conditions file.
    pub fn conditions_file(&self) -> &ConditionsFile {
        &self.conditions
    }

    /// The whole ranges file.
    pub fn ranges_file(&self) -> &RangesFile {
        &self.ranges
    }

    /// The whole titles file.
    pub fn titles_file(&self) -> &TitlesFile {
        &self.titles
    }
}

/// Sorts a file's records into the order the enum declares, and reports any
/// that are missing or duplicated.
///
/// Reordering at load is what lets the accessors index straight into the
/// collection, and it keeps the in-memory order the same however the file was
/// written.
fn canonicalise<Record, Key: Copy + PartialEq + fmt::Debug>(
    file: &'static str,
    field: &str,
    records: &mut Vec<Record>,
    expected: &[Key],
    key_of: impl Fn(&Record) -> Key,
    errors: &mut Vec<LoadError>,
) {
    let mut taken: Vec<Option<Record>> = expected.iter().map(|_| None).collect();
    let mut extra = Vec::new();

    for record in records.drain(..) {
        let key = key_of(&record);
        match expected.iter().position(|candidate| *candidate == key) {
            Some(slot) if taken[slot].is_none() => taken[slot] = Some(record),
            Some(_) => errors.push(LoadError::new(
                file,
                format!("{field} lists {key:?} more than once"),
            )),
            None => extra.push(key),
        }
    }

    for key in extra {
        errors.push(LoadError::new(
            file,
            format!("{field} has a record for {key:?}, which is not expected here"),
        ));
    }

    let missing: Vec<Key> = expected
        .iter()
        .zip(&taken)
        .filter(|(_, slot)| slot.is_none())
        .map(|(key, _)| *key)
        .collect();
    if !missing.is_empty() {
        errors.push(LoadError::new(
            file,
            format!(
                "{field} is missing {} record(s): {missing:?}",
                missing.len()
            ),
        ));
        return;
    }

    *records = taken.into_iter().flatten().collect();
}

/// Reads and parses one file, recording any problem and returning [`None`].
fn read_file<T: serde::de::DeserializeOwned>(
    dir: &Path,
    name: &'static str,
    errors: &mut Vec<LoadError>,
) -> Option<T> {
    let path = dir.join(name);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            errors.push(LoadError {
                file: name,
                path: Some(path),
                message: format!("could not be read: {error}"),
            });
            return None;
        }
    };

    match ron::from_str::<T>(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            errors.push(LoadError {
                file: name,
                path: Some(path),
                message: error.to_string(),
            });
            None
        }
    }
}

/// One thing wrong with a core data file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    /// The file it was found in.
    pub file: &'static str,
    /// Where that file was, when it was read from disk.
    pub path: Option<PathBuf>,
    /// What to fix.
    pub message: String,
}

impl LoadError {
    fn new(file: &'static str, message: impl Into<String>) -> Self {
        LoadError {
            file,
            path: None,
            message: message.into(),
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(f, "{}: {}", path.display(), self.message),
            None => write!(f, "{}: {}", self.file, self.message),
        }
    }
}

/// Everything wrong with a directory of core data files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadErrors(pub Vec<LoadError>);

impl fmt::Display for LoadErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} problem(s) in the core data:", self.0.len())?;
        for error in &self.0 {
            writeln!(f, "  {error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for LoadErrors {}

impl LoadErrors {
    /// Turns these into the report type the rest of the data crate uses.
    ///
    /// The core files are loaded by a different path from the entity tree —
    /// they are cross-checked against the enums in code — but an author
    /// wants one report, not two, so the two kinds of problem are merged.
    ///
    /// `prefix` is the folder the core files sit in relative to the data
    /// root, so that the reported path matches what the author would type.
    pub fn into_data_errors(self, prefix: &str) -> Vec<crate::error::DataError> {
        self.0
            .into_iter()
            .map(|problem| {
                crate::error::DataError::consistency(
                    format!("{prefix}/{}", problem.file),
                    problem.message,
                )
            })
            .collect()
    }
}
