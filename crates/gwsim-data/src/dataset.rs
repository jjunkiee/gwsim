//! Loading a whole `data/` tree (§8.1).

use std::collections::BTreeMap;

use crate::assumptions::{Assumptions, AssumptionsFile};
use crate::error::{DataError, DataErrorKind, DataErrors, Location};
use crate::foe::{Foe, HeroesFile, MinionsFile, SpiritsFile};
use crate::ids::{SkillId, Slug, slugify};
use crate::items::{
    ArmorFile, ConsumablesFile, InsigniasFile, RunesFile, WeaponUpgradesFile, WeaponsFile,
};
use crate::scenario::{Benchmark, Encounter, Situation, SituationSet};
use crate::skill::Skill;
use crate::skill_index::SkillIndexFile;
use crate::source::DataSource;

/// An entity loaded from a file, and the file it came from.
///
/// The path travels with the value because almost every error worth reporting
/// is about a relationship between files, and "which file said this?" has to
/// survive into the check that finds the problem.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry<T> {
    /// The file this came from, relative to the data root.
    pub path: String,
    pub value: T,
}

/// Everything in a `data/` tree.
///
/// Collections are keyed by slug and so iterate in a stable order, which
/// matters because the optimiser's results must not depend on directory
/// listing order (§10.13).
#[derive(Debug, Clone, Default)]
pub struct DataSet {
    pub skills: BTreeMap<Slug, Entry<Skill>>,
    pub foes: BTreeMap<Slug, Entry<Foe>>,
    pub encounters: BTreeMap<Slug, Entry<Encounter>>,
    pub situations: BTreeMap<Slug, Entry<Situation>>,
    pub situation_sets: BTreeMap<Slug, Entry<SituationSet>>,
    pub benchmarks: BTreeMap<Slug, Entry<Benchmark>>,

    /// Every player skill in the game, the coverage denominator (T2.3.6).
    pub skill_index: Option<Entry<SkillIndexFile>>,

    pub armor: Option<Entry<ArmorFile>>,
    pub runes: Option<Entry<RunesFile>>,
    pub insignias: Option<Entry<InsigniasFile>>,
    pub weapons: Option<Entry<WeaponsFile>>,
    pub weapon_upgrades: Option<Entry<WeaponUpgradesFile>>,
    pub consumables: Option<Entry<ConsumablesFile>>,

    pub heroes: Option<Entry<HeroesFile>>,
    pub minions: Option<Entry<MinionsFile>>,
    pub spirits: Option<Entry<SpiritsFile>>,

    pub assumptions: Assumptions,

    /// Skills by template id, for template decoding.
    skills_by_id: BTreeMap<SkillId, Slug>,

    /// Problems that were reported but did not stop the tree loading.
    warnings: DataErrors,

    /// Slugs whose file exists but failed to parse.
    ///
    /// A file that did not parse has already been reported. Without this,
    /// everything referring to it would *also* report "there is no such
    /// file", which is both noise and untrue: the file is right there.
    unparsed: std::collections::BTreeSet<Slug>,
}

impl DataSet {
    /// Reads and checks a whole data tree.
    ///
    /// **Every** problem is collected, never just the first: an author fixing
    /// a seeded file wants the whole list, not one round trip per mistake.
    pub fn load(source: &dyn DataSource) -> Result<DataSet, DataErrors> {
        let mut problems = DataErrors::default();
        let mut data = DataSet::default();

        let paths = match source.list() {
            Ok(paths) => paths,
            Err(error) => {
                problems.0.push(DataError::new(
                    source.describe(),
                    DataErrorKind::Io,
                    format!("the data tree could not be listed: {error}"),
                ));
                return Err(problems);
            }
        };

        for path in paths {
            data.route(source, &path, &mut problems);
        }

        data.check_layout(&mut problems);
        crate::checks::run(&data, &mut problems);

        problems.sort();
        if problems.is_fatal() {
            Err(problems)
        } else {
            data.warnings = problems;
            Ok(data)
        }
    }

    /// Sends one file to the schema that matches its place in the layout.
    fn route(&mut self, source: &dyn DataSource, path: &str, problems: &mut DataErrors) {
        // Documentation and the licence sit alongside the data and are not
        // data themselves (§8.1).
        if path.ends_with(".md") || path == "LICENSE" || path.ends_with("/LICENSE") {
            return;
        }
        if !path.ends_with(".ron") {
            problems.0.push(DataError::layout(
                path,
                "only .ron data files, .md documentation and LICENSE belong under data/",
            ));
            return;
        }

        let text = match source.read(path) {
            Ok(text) => text,
            Err(error) => {
                problems.0.push(DataError::new(
                    path,
                    DataErrorKind::Io,
                    format!("could not be read: {error}"),
                ));
                return;
            }
        };

        let parts: Vec<&str> = path.split('/').collect();
        match parts.as_slice() {
            // Core files are loaded separately by CoreData, which checks them
            // against the enums in code. Skipped rather than rejected.
            ["core", _] => {}

            ["assumptions.ron"] => {
                if let Some(file) = parse::<AssumptionsFile>(path, &text, problems) {
                    self.assumptions = Assumptions::new(file.assumptions);
                }
            }

            ["skills", "index.ron"] => {
                self.skill_index = parse(path, &text, problems).map(|value| entry(path, value));
            }

            ["skills", _folder, _file] => match parse::<Skill>(path, &text, problems) {
                Some(skill) => self.insert_skill(path, skill, problems),
                None => self.note_unparsed(path),
            },

            ["creatures", "foes", _affiliation, _file] => {
                match parse::<Foe>(path, &text, problems) {
                    Some(foe) => insert_by_slug(
                        &mut self.foes,
                        path,
                        slugify(&foe.name),
                        foe,
                        "foe",
                        problems,
                    ),
                    None => self.note_unparsed(path),
                }
            }
            ["creatures", "heroes.ron"] => {
                self.heroes = parse(path, &text, problems).map(|value| entry(path, value));
            }
            ["creatures", "minions.ron"] => {
                self.minions = parse(path, &text, problems).map(|value| entry(path, value));
            }
            ["creatures", "spirits.ron"] => {
                self.spirits = parse(path, &text, problems).map(|value| entry(path, value));
            }

            ["items", "armor.ron"] => {
                self.armor = parse(path, &text, problems).map(|value| entry(path, value));
            }
            ["items", "runes.ron"] => {
                self.runes = parse(path, &text, problems).map(|value| entry(path, value));
            }
            ["items", "insignias.ron"] => {
                self.insignias = parse(path, &text, problems).map(|value| entry(path, value));
            }
            ["items", "weapons.ron"] => {
                self.weapons = parse(path, &text, problems).map(|value| entry(path, value));
            }
            ["items", "weapon_upgrades.ron"] => {
                self.weapon_upgrades = parse(path, &text, problems).map(|value| entry(path, value));
            }
            ["items", "consumables.ron"] => {
                self.consumables = parse(path, &text, problems).map(|value| entry(path, value));
            }

            ["encounters", "generic", _] | ["encounters", "curated", _, _, _] => {
                match parse::<Encounter>(path, &text, problems) {
                    Some(encounter) => insert_by_slug(
                        &mut self.encounters,
                        path,
                        slugify(&encounter.name),
                        encounter,
                        "encounter",
                        problems,
                    ),
                    None => self.note_unparsed(path),
                }
            }

            ["situations", _file] => match parse::<Situation>(path, &text, problems) {
                Some(situation) => insert_by_slug(
                    &mut self.situations,
                    path,
                    slugify(&situation.name),
                    situation,
                    "situation",
                    problems,
                ),
                None => self.note_unparsed(path),
            },
            ["situation_sets", _file] => {
                if let Some(set) = parse::<SituationSet>(path, &text, problems) {
                    insert_by_slug(
                        &mut self.situation_sets,
                        path,
                        slugify(&set.name),
                        set,
                        "situation set",
                        problems,
                    );
                }
            }
            ["benchmarks", _file] => {
                if let Some(benchmark) = parse::<Benchmark>(path, &text, problems) {
                    insert_by_slug(
                        &mut self.benchmarks,
                        path,
                        slugify(&benchmark.name),
                        benchmark,
                        "benchmark",
                        problems,
                    );
                }
            }

            _ => problems.0.push(DataError::layout(
                path,
                "this path is not part of the data layout; see docs/data-authoring.md \
                 for where files belong",
            )),
        }
    }

    fn insert_skill(&mut self, path: &str, skill: Skill, problems: &mut DataErrors) {
        let slug = slugify(&skill.name);
        let id = skill.id;

        if let Some(existing) = self.skills_by_id.get(&id) {
            if *existing != slug {
                let existing_path = self
                    .skills
                    .get(existing)
                    .map(|entry| entry.path.as_str())
                    .unwrap_or("another file");
                problems.0.push(DataError::consistency(
                    path,
                    format!(
                        "skill id {id} is already used by {existing_path}; template ids \
                         are unique, so one of the two is wrong"
                    ),
                ));
            }
        } else if id.get() != SkillId::NONE {
            self.skills_by_id.insert(id, slug.clone());
        }

        insert_by_slug(&mut self.skills, path, slug, skill, "skill", problems);
    }

    /// Checks that each file sits where its contents say it should.
    fn check_layout(&self, problems: &mut DataErrors) {
        for (slug, entry) in &self.skills {
            check_file_name(&entry.path, slug, problems);

            // A skill's folder must match its profession, so that the tree can
            // be browsed by profession and so a misfiled skill is caught here
            // rather than by someone wondering where it went.
            let folder = entry.path.split('/').nth(1).unwrap_or("");
            let expected = match entry.value.profession {
                // PvE-only skills sit outside every profession's pool, so they
                // all live in common/, even the ones a profession owns (such as
                // the Kurzick and Luxon allegiance skills). That rule belongs to
                // the pve_only check in checks.rs; enforcing the profession
                // folder here as well made the two contradict each other
                // (fallout F2.8).
                Some(_) if entry.value.pve_only => folder.to_owned(),
                Some(profession) => format!("{profession:?}").to_lowercase(),
                None => {
                    if folder == "common" || folder == "monster" {
                        folder.to_owned()
                    } else {
                        "common".to_owned()
                    }
                }
            };
            if folder != expected {
                problems.0.push(DataError::layout(
                    &entry.path,
                    format!(
                        "this is a {} skill, so it belongs in skills/{expected}/, not \
                         skills/{folder}/",
                        entry
                            .value
                            .profession
                            .map(|profession| format!("{profession:?}"))
                            .unwrap_or_else(|| "profession-less".to_owned()),
                    ),
                ));
            }
        }

        for (slug, entry) in &self.foes {
            check_file_name(&entry.path, slug, problems);

            let folder = entry.path.split('/').nth(2).unwrap_or("");
            let expected = slugify(&entry.value.affiliation);
            if folder != expected.as_str() {
                problems.0.push(DataError::layout(
                    &entry.path,
                    format!(
                        "this foe's affiliation is {:?}, so it belongs in \
                         creatures/foes/{expected}/, not creatures/foes/{folder}/",
                        entry.value.affiliation
                    ),
                ));
            }
        }

        for (slug, entry) in &self.encounters {
            check_file_name(&entry.path, slug, problems);
        }
        for (slug, entry) in &self.situations {
            check_file_name(&entry.path, slug, problems);
        }
        for (slug, entry) in &self.situation_sets {
            check_file_name(&entry.path, slug, problems);
        }
        for (slug, entry) in &self.benchmarks {
            check_file_name(&entry.path, slug, problems);
        }
    }

    /// Records that a file could not be parsed, keyed by the slug its name
    /// implies. The layout rule that a file is named after its contents is
    /// what makes this reliable.
    fn note_unparsed(&mut self, path: &str) {
        let stem = path
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".ron"))
            .unwrap_or_default();
        if let Ok(slug) = stem.parse::<Slug>() {
            self.unparsed.insert(slug);
        }
    }

    /// Whether a slug names a file that exists but failed to parse.
    ///
    /// Reference checks use this to stay quiet about something that has
    /// already been reported for a better reason.
    pub fn is_unparsed(&self, slug: &Slug) -> bool {
        self.unparsed.contains(slug)
    }

    /// The slug of the skill with a template id.
    pub fn skill_slug_for_id(&self, id: SkillId) -> Option<&Slug> {
        self.skills_by_id.get(&id)
    }

    /// A skill by its template id.
    pub fn skill_by_id(&self, id: SkillId) -> Option<&Skill> {
        self.skills_by_id
            .get(&id)
            .and_then(|slug| self.skills.get(slug))
            .map(|entry| &entry.value)
    }

    /// A skill by slug.
    pub fn skill(&self, slug: &Slug) -> Option<&Skill> {
        self.skills.get(slug).map(|entry| &entry.value)
    }

    /// A foe by slug.
    pub fn foe(&self, slug: &Slug) -> Option<&Foe> {
        self.foes.get(slug).map(|entry| &entry.value)
    }

    /// How many entities of each kind were loaded.
    pub fn counts(&self) -> DataCounts {
        DataCounts {
            skills: self.skills.len(),
            foes: self.foes.len(),
            encounters: self.encounters.len(),
            situations: self.situations.len(),
            situation_sets: self.situation_sets.len(),
            benchmarks: self.benchmarks.len(),
            assumptions: self.assumptions.len(),
        }
    }

    /// Problems that did not stop the tree loading.
    pub fn warnings(&self) -> &DataErrors {
        &self.warnings
    }
}

/// How many of each kind of entity a data set holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct DataCounts {
    pub skills: usize,
    pub foes: usize,
    pub encounters: usize,
    pub situations: usize,
    pub situation_sets: usize,
    pub benchmarks: usize,
    pub assumptions: usize,
}

fn entry<T>(path: &str, value: T) -> Entry<T> {
    Entry {
        path: path.to_owned(),
        value,
    }
}

/// Inserts an entity, reporting a duplicate by naming **both** files.
fn insert_by_slug<T>(
    map: &mut BTreeMap<Slug, Entry<T>>,
    path: &str,
    slug: Slug,
    value: T,
    kind: &str,
    problems: &mut DataErrors,
) {
    if let Some(existing) = map.get(&slug) {
        problems.0.push(DataError::consistency(
            path,
            format!(
                "this {kind} has the same name as the one in {}; every {kind} needs a \
                 distinct name, because the name is its key",
                existing.path
            ),
        ));
        return;
    }
    map.insert(slug, entry(path, value));
}

/// Checks that a file is named after what it contains.
fn check_file_name(path: &str, slug: &Slug, problems: &mut DataErrors) {
    let stem = path
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".ron"))
        .unwrap_or_default();
    if stem != slug.as_str() {
        problems.0.push(DataError::layout(
            path,
            format!(
                "the file is named {stem}.ron but its contents slugify to {slug}; \
                 rename it to {slug}.ron"
            ),
        ));
    }
}

/// Parses one file, turning any failure into a reportable problem.
///
/// On failure the text is parsed a second time through `serde_path_to_error`,
/// purely to recover a field path. That costs nothing that matters: the file
/// is already being rejected (T1.2.1 §5).
fn parse<T: serde::de::DeserializeOwned>(
    path: &str,
    text: &str,
    problems: &mut DataErrors,
) -> Option<T> {
    match ron::from_str::<T>(text) {
        Ok(value) => Some(value),
        Err(error) => {
            let at = Location {
                line: error.span.start.line as u32,
                col: error.span.start.col as u32,
            };
            let field_path = field_path_of::<T>(text);

            // A position-only failure with no structure to it is a syntax
            // error; anything serde could name a field for is a schema error.
            let kind = if field_path.is_some() || is_schema_error(&error.code) {
                DataErrorKind::Schema { at, field_path }
            } else {
                DataErrorKind::Syntax { at }
            };

            problems
                .0
                .push(DataError::new(path, kind, describe(&error.code)));
            None
        }
    }
}

/// Re-parses to recover the field path, and nothing else.
fn field_path_of<T: serde::de::DeserializeOwned>(text: &str) -> Option<String> {
    let mut deserializer = ron::Deserializer::from_str(text).ok()?;
    match serde_path_to_error::deserialize::<_, T>(&mut deserializer) {
        Ok(_) => None,
        Err(error) => {
            let path = error.path().to_string();
            // serde_path_to_error writes "." for the root and "?" for a place
            // it cannot name. Neither tells a reader anything they did not
            // already know from the file name and the line number.
            (path != "." && path != "?" && !path.is_empty()).then_some(path)
        }
    }
}

/// Whether an error is about the shape of the data rather than the syntax.
fn is_schema_error(code: &ron::Error) -> bool {
    matches!(
        code,
        ron::Error::NoSuchStructField { .. }
            | ron::Error::MissingStructField { .. }
            | ron::Error::DuplicateStructField { .. }
            | ron::Error::NoSuchEnumVariant { .. }
    )
}

/// Turns a RON error into a message that says what to do.
///
/// RON's own text is already good; this adds a suggestion where the structured
/// error gives us the material for one (T1.2.1 §2).
fn describe(code: &ron::Error) -> String {
    match code {
        ron::Error::NoSuchStructField {
            expected, found, ..
        } => {
            let mut message = format!("there is no field named `{found}` here");
            if let Some(suggestion) = closest(found, expected) {
                message.push_str(&format!("; did you mean `{suggestion}`?"));
            } else if !expected.is_empty() {
                message.push_str(&format!("; the fields here are {}", list(expected)));
            }
            message
        }
        ron::Error::NoSuchEnumVariant {
            expected, found, ..
        } => {
            let mut message = format!("`{found}` is not one of the allowed values");
            if let Some(suggestion) = closest(found, expected) {
                message.push_str(&format!("; did you mean `{suggestion}`?"));
            } else if !expected.is_empty() {
                message.push_str(&format!("; the allowed values are {}", list(expected)));
            }
            message
        }
        ron::Error::MissingStructField { field, .. } => {
            format!("the field `{field}` is required but missing")
        }
        other => other.to_string(),
    }
}

/// The candidate closest to what was written, if one is close enough to be
/// worth suggesting.
fn closest(found: &str, candidates: &[&str]) -> Option<String> {
    let lower = found.to_lowercase();

    // A candidate the typed name is a prefix of wins outright. Without this,
    // `Domination` suggests `Motivation` — three edits away — over
    // `DominationMagic`, which is five but is obviously what was meant.
    let prefix_match = candidates
        .iter()
        .filter(|candidate| {
            let candidate = candidate.to_lowercase();
            candidate.starts_with(&lower) || lower.starts_with(&candidate)
        })
        .min_by_key(|candidate| candidate.len());
    if let Some(candidate) = prefix_match {
        return Some((*candidate).to_owned());
    }

    candidates
        .iter()
        .map(|candidate| (edit_distance(&lower, &candidate.to_lowercase()), *candidate))
        // A third of the length, so short names need a closer match than long
        // ones and unrelated words are not "suggested".
        .filter(|(distance, candidate)| *distance <= (candidate.len() / 3).max(1))
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate.to_owned())
}

/// Levenshtein distance, used only to suggest a correction.
fn edit_distance(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (i, left_char) in left.iter().enumerate() {
        current[0] = i + 1;
        for (j, right_char) in right.iter().enumerate() {
            let substitution = usize::from(left_char != right_char);
            current[j + 1] = (previous[j] + substitution)
                .min(previous[j + 1] + 1)
                .min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

/// Renders a list of names for a message.
fn list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => format!("`{only}`"),
        [first, second] => format!("`{first}` and `{second}`"),
        [rest @ .., last] => {
            let head: Vec<String> = rest.iter().map(|name| format!("`{name}`")).collect();
            format!("{}, and `{last}`", head.join(", "))
        }
    }
}
