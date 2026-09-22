//! Cross-reference and consistency checks over a whole data tree (§8.9).
//!
//! These are the checks that need more than one file, or that compare a file
//! against a rule no schema can express. Everything here reports rather than
//! stops, so one run tells an author the whole story.

use crate::dataset::{DataSet, Entry};
use crate::error::{DataError, DataErrors};
use crate::foe::SkillRef;
use crate::ids::{AssumptionId, Slug};
use crate::provenance::{Provenance, ReviewStatus};
use crate::scenario::SituationEncounters;
use crate::skill::Skill;

/// Somewhere handler names can be checked against.
///
/// The engine owns the handlers, and the data crate must not depend on the
/// engine (ENG-1 keeps the dependency pointing one way), so the caller passes
/// this in. `gwsim data validate` supplies the real registry; tests supply a
/// list.
pub trait HandlerRegistry {
    /// Whether a handler of this name exists.
    fn contains(&self, name: &str) -> bool;

    /// Every registered name, for suggesting a correction.
    fn names(&self) -> Vec<String>;
}

/// Lets a plain slice of names stand in for a registry in tests, and for the
/// CLI before the engine's real registry exists.
impl HandlerRegistry for &[&str] {
    fn contains(&self, name: &str) -> bool {
        // Spelled out rather than `self.contains(&name)`, which would resolve
        // to the slice's inherent method and read as though it recursed.
        self.iter().copied().any(|known| known == name)
    }

    fn names(&self) -> Vec<String> {
        self.iter().map(|name| (*name).to_owned()).collect()
    }
}

/// Runs every check that does not need the engine.
pub fn run(data: &DataSet, problems: &mut DataErrors) {
    check_references(data, problems);
    check_skills(data, problems);
    crate::checks_dsl::check_encodings(data, problems);
    check_foes(data, problems);
    check_benchmarks(data, problems);
    check_provenance(data, problems);
    check_assumptions(data, problems);
    check_warnings(data, problems);
}

/// Checks handler names. Separate because it needs something the engine owns.
pub fn check_handlers(data: &DataSet, registry: &dyn HandlerRegistry) -> DataErrors {
    let mut problems = DataErrors::default();
    for entry in data.skills.values() {
        let Some(handler) = entry
            .value
            .encoding
            .as_ref()
            .and_then(|encoding| encoding.handler.as_ref())
        else {
            continue;
        };
        if !registry.contains(&handler.name) {
            let known = registry.names();
            let hint = if known.is_empty() {
                String::new()
            } else {
                format!("; registered handlers are {}", known.join(", "))
            };
            problems.0.push(DataError::reference(
                &entry.path,
                format!(
                    "no handler is registered under the name `{}`{hint}",
                    handler.name
                ),
            ));
        }
    }
    problems.sort();
    problems
}

// ------------------------------------------------------------- references

fn check_references(data: &DataSet, problems: &mut DataErrors) {
    // Foes' skill bars.
    for entry in data.foes.values() {
        let mut bars: Vec<&crate::foe::FoeSkill> = entry.value.skills.iter().collect();
        for variant in &entry.value.variants {
            if let Some(skills) = &variant.skills {
                bars.extend(skills.iter());
            }
        }
        for foe_skill in bars {
            match &foe_skill.skill {
                SkillRef::Slug(slug) => {
                    // A skill whose own file failed to parse has already been
                    // reported; saying "there is no such file" as well would
                    // be noise, and untrue.
                    if !data.skills.contains_key(slug) && !data.is_unparsed(slug) {
                        problems.0.push(DataError::reference(
                            &entry.path,
                            format!(
                                "this foe uses the skill `{slug}`, but there is no \
                                 skills/**/{slug}.ron"
                            ),
                        ));
                    }
                }
                SkillRef::Id(id) => {
                    if data.skill_slug_for_id(*id).is_none() {
                        problems.0.push(DataError::reference(
                            &entry.path,
                            format!(
                                "this foe uses skill id {id}, but no skill file claims \
                                 that id"
                            ),
                        ));
                    }
                }
            }
        }
    }

    // Encounters' foes.
    for entry in data.encounters.values() {
        for group in &entry.value.groups {
            for group_foe in &group.foes {
                if !data.foes.contains_key(&group_foe.foe) {
                    if data.is_unparsed(&group_foe.foe) {
                        continue;
                    }
                    problems.0.push(DataError::reference(
                        &entry.path,
                        format!(
                            "this encounter uses the foe `{}`, but there is no \
                             creatures/foes/**/{}.ron",
                            group_foe.foe, group_foe.foe
                        ),
                    ));
                    continue;
                }
                // A named variant has to exist on that foe.
                if let Some(variant) = &group_foe.variant {
                    let foe = data.foe(&group_foe.foe).expect("just checked");
                    if !foe.variants.iter().any(|known| known.name == *variant) {
                        let known: Vec<&str> = foe
                            .variants
                            .iter()
                            .map(|variant| variant.name.as_str())
                            .collect();
                        let hint = if known.is_empty() {
                            "that foe has no variants".to_owned()
                        } else {
                            format!("its variants are {}", known.join(", "))
                        };
                        problems.0.push(DataError::reference(
                            &entry.path,
                            format!(
                                "the foe `{}` has no variant named `{variant}`; {hint}",
                                group_foe.foe
                            ),
                        ));
                    }
                }
            }
        }
    }

    // Situations' encounters.
    for entry in data.situations.values() {
        let referenced: Vec<&Slug> = match &entry.value.encounters {
            SituationEncounters::Single(slug) => vec![slug],
            SituationEncounters::Chain(steps) => steps.iter().map(|step| &step.encounter).collect(),
        };
        for slug in referenced {
            if !data.encounters.contains_key(slug) && !data.is_unparsed(slug) {
                problems.0.push(DataError::reference(
                    &entry.path,
                    format!(
                        "this situation uses the encounter `{slug}`, but there is no \
                         encounters/**/{slug}.ron"
                    ),
                ));
            }
        }

        for slug in &entry.value.consumables {
            let known = data.consumables.as_ref().is_some_and(|file| {
                file.value
                    .consumables
                    .iter()
                    .any(|consumable| consumable.slug == *slug)
            });
            if !known {
                problems.0.push(DataError::reference(
                    &entry.path,
                    format!(
                        "this situation uses the consumable `{slug}`, which is not in \
                         items/consumables.ron"
                    ),
                ));
            }
        }
    }

    // Situation sets' situations.
    for entry in data.situation_sets.values() {
        for set_entry in &entry.value.entries {
            if !data.situations.contains_key(&set_entry.situation)
                && !data.is_unparsed(&set_entry.situation)
            {
                problems.0.push(DataError::reference(
                    &entry.path,
                    format!(
                        "this set uses the situation `{}`, but there is no \
                         situations/{}.ron",
                        set_entry.situation, set_entry.situation
                    ),
                ));
            }
        }
        if set_entry_weights_are_unusable(entry) {
            problems.0.push(DataError::consistency(
                &entry.path,
                "every weight in this set is zero or negative, so no situation would \
                 count for anything",
            ));
        }
    }
}

fn set_entry_weights_are_unusable(entry: &Entry<crate::scenario::SituationSet>) -> bool {
    !entry.value.entries.is_empty() && entry.value.total_weight() <= 0.0
}

// ----------------------------------------------------------------- skills

fn check_skills(data: &DataSet, problems: &mut DataErrors) {
    for entry in data.skills.values() {
        let skill = &entry.value;
        let folder = entry.path.split('/').nth(1).unwrap_or("");

        // PvE-only skills are not part of any profession's normal pool, so
        // they live in skills/common/ whatever profession they belong to.
        if skill.pve_only && folder != "common" {
            problems.0.push(DataError::consistency(
                &entry.path,
                "this skill is marked pve_only, so it belongs in skills/common/",
            ));
        }

        // A title track scales a skill only through the PvE-only mechanism.
        if skill.title_track.is_some() && !skill.pve_only {
            problems.0.push(DataError::consistency(
                &entry.path,
                "this skill has a title_track but is not pve_only; only PvE-only \
                 skills scale on a title",
            ));
        }

        // Monster skills are not normally elite, so an elite one is either a
        // mistake or something worth explaining.
        if skill.elite && folder == "monster" && skill.provenance.notes.trim().is_empty() {
            problems.0.push(DataError::consistency(
                &entry.path,
                "this monster skill is marked elite; if that is right, say why in \
                 provenance.notes",
            ));
        }

        // A skill that scales must have something to scale on.
        let scales = skill
            .extracted
            .scaled
            .iter()
            .any(|number| number.r0 != number.r15);
        if scales && skill.attribute.is_none() && skill.title_track.is_none() {
            problems.0.push(DataError::consistency(
                &entry.path,
                "this skill has values that change with rank but no attribute or \
                 title_track to scale on",
            ));
        }

        check_review_status(entry, problems);
        check_scaled_numbers(entry, problems);
    }
}

/// The review status has to match what the file actually contains (§8.7).
fn check_review_status(entry: &Entry<Skill>, problems: &mut DataErrors) {
    let skill = &entry.value;
    match skill.provenance.review {
        ReviewStatus::NumbersOnly => {
            if skill.encoding.is_some() {
                problems.0.push(DataError::consistency(
                    &entry.path,
                    "this skill is NumbersOnly but has an encoding; raise it to Draft, \
                     or remove the encoding",
                ));
            }
        }
        ReviewStatus::Draft | ReviewStatus::Reviewed => {
            if skill.encoding.is_none() {
                problems.0.push(DataError::consistency(
                    &entry.path,
                    format!(
                        "this skill is {:?} but has no encoding; a {:?} skill is one \
                         that has been encoded, so set it back to NumbersOnly",
                        skill.provenance.review, skill.provenance.review
                    ),
                ));
            }
        }
    }
}

/// The wiki publishes three points on every scaled value, and they have to
/// agree with the rounding rule (§17.3).
fn check_scaled_numbers(entry: &Entry<Skill>, problems: &mut DataErrors) {
    for number in &entry.value.extracted.scaled {
        if number.r12_is_consistent() {
            continue;
        }
        let message = format!(
            "the value `{}` is {} at rank 12, but {} at rank 0 and {} at rank 15 give \
             {} under the usual rounding",
            number.label,
            number.r12,
            number.r0,
            number.r15,
            number.expected_r12()
        );
        // A skill that genuinely rounds differently says so, and then the
        // mismatch is worth noting but not worth refusing to load over.
        let problem = if number.special_rounding {
            DataError::consistency(
                &entry.path,
                format!("{message}; accepted because special_rounding is set"),
            )
            .as_warning()
        } else {
            DataError::consistency(
                &entry.path,
                format!(
                    "{message}; check the wiki, and set special_rounding if the skill \
                     really does round differently"
                ),
            )
        };
        problems.0.push(problem);
    }
}

// -------------------------------------------------------------------- foes

fn check_foes(data: &DataSet, problems: &mut DataErrors) {
    for entry in data.foes.values() {
        let foe = &entry.value;

        // Without a hard-mode level the engine would have to guess, and the
        // level mapping in levels.ron has gaps and overlaps that make guessing
        // unsafe. The foe file is the right place to settle it.
        if !foe.level.has_hm() {
            problems.0.push(DataError::consistency(
                &entry.path,
                "this foe has no hard-mode level; take it from the mapping in \
                 core/levels.ron and write it here",
            ));
        }

        // Variant names are how an encounter picks one, so they must differ.
        let mut names: Vec<&str> = foe
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        if names.len() != before {
            problems.0.push(DataError::consistency(
                &entry.path,
                "two of this foe's variants share a name; an encounter picks a variant \
                 by name, so they have to differ",
            ));
        }

        // Spirits and fleshy creatures are opposites, and a foe marked both
        // would take conditions it should be immune to.
        let traits = &foe.traits;
        if traits.contains(&crate::foe::CreatureTrait::Spirit)
            && traits.contains(&crate::foe::CreatureTrait::Fleshy)
        {
            problems.0.push(DataError::consistency(
                &entry.path,
                "this foe is marked both Spirit and Fleshy; spirits are not fleshy, and \
                 the two give opposite condition immunities",
            ));
        }
    }
}

// -------------------------------------------------------------- benchmarks

/// Checks that a benchmark's published template codes decode (T1.2.8).
///
/// Only decoding is checked here. Whether gwsim *has* the skills a benchmark
/// names is a question about coverage, not about whether this file is valid,
/// and it lives in [`crate::coverage`] — `data/skills/` is empty until P2 and
/// stays incomplete until P7, so asking it here would attach a warning to
/// every benchmark on every run for the length of the project. A warning that
/// is always on teaches people to ignore warnings.
fn check_benchmarks(data: &DataSet, problems: &mut DataErrors) {
    for entry in data.benchmarks.values() {
        for (index, slot) in entry.value.slots.iter().enumerate() {
            let slot_number = index + 1;

            if let Err(error) = crate::template::SkillTemplate::decode(&slot.skill_code) {
                problems.0.push(DataError::consistency(
                    &entry.path,
                    format!("slot {slot_number}'s skill_code does not decode: {error}"),
                ));
            }

            if let Some(code) = &slot.equipment_code
                && let Err(error) = crate::template::EquipmentTemplate::decode(code)
            {
                problems.0.push(DataError::consistency(
                    &entry.path,
                    format!("slot {slot_number}'s equipment_code does not decode: {error}"),
                ));
            }
        }
    }
}

// ------------------------------------------------------------- provenance

fn check_provenance(data: &DataSet, problems: &mut DataErrors) {
    let blocks: Vec<(&str, &Provenance)> = data
        .skills
        .values()
        .map(|entry| (entry.path.as_str(), &entry.value.provenance))
        .chain(
            data.foes
                .values()
                .map(|entry| (entry.path.as_str(), &entry.value.provenance)),
        )
        .chain(
            data.encounters
                .values()
                .map(|entry| (entry.path.as_str(), &entry.value.provenance)),
        )
        .chain(
            data.situation_sets
                .values()
                .map(|entry| (entry.path.as_str(), &entry.value.provenance)),
        )
        .chain(
            data.benchmarks
                .values()
                .map(|entry| (entry.path.as_str(), &entry.value.provenance)),
        )
        .collect();

    for (path, provenance) in blocks {
        for problem in provenance.problems() {
            problems.0.push(DataError::consistency(path, problem));
        }
    }
}

// ------------------------------------------------------------ assumptions

fn check_assumptions(data: &DataSet, problems: &mut DataErrors) {
    for (path, ids) in assumption_references(data) {
        for id in ids {
            if !data.assumptions.contains(id) {
                problems.0.push(DataError::reference(
                    path,
                    format!(
                        "this file cites {id}, which is not in data/assumptions.ron; \
                         add it to the register or correct the id"
                    ),
                ));
            }
        }
    }

    // The register itself must be in order and free of duplicates, because a
    // duplicate id means one of the two entries is silently unreachable.
    let entries = data.assumptions.all();
    for pair in entries.windows(2) {
        if pair[0].id == pair[1].id {
            problems.0.push(DataError::consistency(
                "assumptions.ron",
                format!("{} appears twice in the register", pair[0].id),
            ));
        } else if pair[0].id > pair[1].id {
            problems.0.push(DataError::consistency(
                "assumptions.ron",
                format!(
                    "{} comes after {} in the file; the register is kept in id order",
                    pair[1].id, pair[0].id
                ),
            ));
        }
    }
}

/// Every assumption id cited anywhere, with the file that cites it.
fn assumption_references(data: &DataSet) -> Vec<(&str, Vec<AssumptionId>)> {
    let cited = data
        .skills
        .values()
        .map(|entry| (entry.path.as_str(), &entry.value.provenance.assumptions))
        .chain(
            data.foes
                .values()
                .map(|entry| (entry.path.as_str(), &entry.value.provenance.assumptions)),
        )
        .chain(
            data.encounters
                .values()
                .map(|entry| (entry.path.as_str(), &entry.value.provenance.assumptions)),
        )
        .chain(
            data.situations
                .values()
                .map(|entry| (entry.path.as_str(), &entry.value.assumption_refs)),
        )
        .chain(
            data.situation_sets
                .values()
                .map(|entry| (entry.path.as_str(), &entry.value.provenance.assumptions)),
        )
        .chain(
            data.benchmarks
                .values()
                .map(|entry| (entry.path.as_str(), &entry.value.provenance.assumptions)),
        );

    cited
        .filter(|(_, ids)| !ids.is_empty())
        .map(|(path, ids)| (path, ids.clone()))
        .collect()
}

// --------------------------------------------------------------- warnings

fn check_warnings(data: &DataSet, problems: &mut DataErrors) {
    // A file leaning on an assumption nobody has settled is usable, but the
    // result it produces is only as good as the guess.
    for (path, ids) in assumption_references(data) {
        for id in ids {
            if data
                .assumptions
                .get(id)
                .is_some_and(|assumption| assumption.is_pending())
            {
                problems.0.push(
                    DataError::consistency(
                        path,
                        format!(
                            "{id} has no value yet, so anything depending on this file \
                             is uncalibrated"
                        ),
                    )
                    .as_warning(),
                );
            }
        }
    }

    // An encounter whose foes carry unencoded skills will run, but those
    // skills will do nothing (§12.6).
    for entry in data.encounters.values() {
        let mut unencoded: Vec<String> = Vec::new();
        for group in &entry.value.groups {
            for group_foe in &group.foes {
                let Some(foe) = data.foe(&group_foe.foe) else {
                    continue;
                };
                for foe_skill in &foe.skills {
                    let SkillRef::Slug(slug) = &foe_skill.skill else {
                        continue;
                    };
                    let Some(skill) = data.skill(slug) else {
                        continue;
                    };
                    if skill.provenance.review == ReviewStatus::NumbersOnly {
                        unencoded.push(slug.to_string());
                    }
                }
            }
        }
        unencoded.sort();
        unencoded.dedup();
        if !unencoded.is_empty() {
            problems.0.push(
                DataError::consistency(
                    &entry.path,
                    format!(
                        "{} of this encounter's foe skills are still NumbersOnly and \
                         will do nothing: {}",
                        unencoded.len(),
                        unencoded.join(", ")
                    ),
                )
                .as_warning(),
            );
        }
    }
}
