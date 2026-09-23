//! T4.1.1: generated checks over every encoded skill (DESIGN §17.3).
//!
//! For each `Draft` or `Reviewed` skill in `data/`:
//!
//! - every `Scaled(at0, at15)` in its encoding matches one of the numbers
//!   the extractor read from the wiki, at ranks 0, 12 and 15 (a skill whose
//!   row is marked `special_rounding` may be one off at rank 12, and a
//!   degeneration written as a negative matches its positive row);
//! - its description renders, with no gaps;
//! - its handler, if any, is registered.
//!
//! Every problem prints on its own line, so a batch sees them all at once.

use std::path::PathBuf;

use gwsim_data::DataSet;
use gwsim_data::derived::scaled;
use gwsim_data::describe::{DescribeContext, describe};
use gwsim_data::provenance::ReviewStatus;
use gwsim_data::skill::Skill;
use gwsim_data::source::DirSource;
use gwsim_engine::handlers::HandlerRegistry;

fn data() -> DataSet {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    DataSet::load(&DirSource::new(dir)).expect("data/ loads")
}

/// Every `Scaled(at0, at15)` written in an encoding, found in its RON form so
/// no construct is missed. `ScaledBy` and `TitleScaled` scale on something
/// else and are not compared.
fn scaled_values(skill: &Skill) -> Vec<(i32, i32)> {
    let Some(encoding) = &skill.encoding else {
        return Vec::new();
    };
    let text = ron::to_string(encoding).expect("an encoding serialises");
    let mut found = Vec::new();
    let mut rest = text.as_str();
    while let Some(at) = rest.find("Scaled(") {
        let before = rest[..at].chars().last();
        let after = &rest[at + "Scaled(".len()..];
        rest = after;
        if before.is_some_and(|c| c.is_ascii_alphabetic()) {
            continue;
        }
        let Some(end) = after.find(')') else { continue };
        let numbers: Vec<i32> = after[..end]
            .split(',')
            .filter_map(|n| n.trim().parse().ok())
            .collect();
        if let [at0, at15] = numbers[..] {
            found.push((at0, at15));
        }
    }
    found
}

fn problems_with(skill: &Skill, slug: &str, handlers: &HandlerRegistry) -> Vec<String> {
    let mut problems = Vec::new();

    for (at0, at15) in scaled_values(skill) {
        let at = |rank| scaled(at0, at15, rank);
        let matches = skill.extracted.scaled.iter().any(|row| {
            let negative = at0 < 0 || at15 < 0;
            let (r0, r12, r15) = if negative && row.r15 > 0 {
                (-row.r0, -row.r12, -row.r15)
            } else {
                (row.r0, row.r12, row.r15)
            };
            let tolerance = if row.special_rounding { 1 } else { 0 };
            at(0) == r0 && at(15) == r15 && (at(12) - r12).abs() <= tolerance
        });
        if !matches {
            problems.push(format!(
                "{slug}: Scaled({at0}, {at15}) gives {}…{}…{}, which matches no extracted row",
                at(0),
                at(12),
                at(15)
            ));
        }
    }

    let context = DescribeContext {
        rank: None,
        title_rank: None,
        core: None,
        handlers,
    };
    let text = describe(skill, &context);
    if text.trim().is_empty() || text.contains("{") || text.contains("  ") {
        problems.push(format!(
            "{slug}: the description does not render cleanly: {text:?}"
        ));
    }

    if let Some(handler) = skill.encoding.as_ref().and_then(|e| e.handler.as_ref())
        && handlers.index_of(&handler.name).is_none()
    {
        problems.push(format!(
            "{slug}: handler {:?} is not registered",
            handler.name
        ));
    }
    problems
}

#[test]
fn every_encoded_skill_matches_its_wiki_numbers_and_renders() {
    let data = data();
    let handlers = HandlerRegistry::standard();
    let mut checked = 0;
    let mut problems = Vec::new();
    for (slug, entry) in &data.skills {
        let skill = &entry.value;
        if !matches!(
            skill.provenance.review,
            ReviewStatus::Draft | ReviewStatus::Reviewed
        ) {
            continue;
        }
        checked += 1;
        problems.extend(problems_with(skill, slug.as_str(), &handlers));
    }
    assert!(checked >= 8, "only {checked} encoded skills found");
    assert!(
        problems.is_empty(),
        "{} problem(s) in {checked} skills:\n{}",
        problems.len(),
        problems.join("\n")
    );
}
