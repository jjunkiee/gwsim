//! `gwsim data coverage` — how much of the game the data covers.

use std::io::{self, Write};

use gwsim_data::coverage::{Coverage, StatusCounts};

use crate::CoverageArgs;
use crate::data::{FAILED, OK};

/// Runs `gwsim data coverage`.
pub fn run(args: &CoverageArgs, out: &mut impl Write) -> io::Result<i32> {
    let loaded = match crate::loading::load(args.data_dir.as_deref()) {
        Ok(loaded) => loaded,
        Err((origin, problems)) => {
            writeln!(out, "could not read {origin}:")?;
            writeln!(out, "{problems}")?;
            return Ok(FAILED);
        }
    };
    let data = loaded.data;

    let coverage = Coverage::compute(&data);

    if args.json {
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&as_json(&coverage))?
        )?;
    } else {
        write_text(&coverage, args, out)?;
    }

    Ok(OK)
}

fn write_text(coverage: &Coverage, args: &CoverageArgs, out: &mut impl Write) -> io::Result<()> {
    if coverage.is_empty() {
        writeln!(out, "No skills have been written yet.")?;
        return Ok(());
    }

    writeln!(
        out,
        "{:<16} {:>10} {:>7} {:>9} {:>7} {:>9}",
        "profession", "NumbersOnly", "Draft", "Reviewed", "total", "encoded"
    )?;

    let wanted = args.profession.as_deref();
    for (profession, counts) in &coverage.by_profession {
        let name = profession
            .map(|profession| format!("{profession:?}"))
            .unwrap_or_else(|| "common".to_owned());
        if let Some(wanted) = wanted
            && !name.eq_ignore_ascii_case(wanted)
        {
            continue;
        }
        write_row(&name, counts, out)?;
    }

    if wanted.is_none() {
        writeln!(out)?;
        write_row("all", &coverage.total, out)?;

        // Saying what the percentage is of matters: until T2.3.6 adds the
        // skill index there is no list of every skill in the game, so
        // "encoded" is a share of the files that exist, which is not the
        // same as a share of the game.
        if !coverage.denominator_is_complete {
            writeln!(
                out,
                "\nPercentages are of the skill files that exist, not of every skill in \
                 the game. The full denominator arrives with data/skills/index.ron \
                 (T2.3.6)."
            )?;
        }
    }

    if !coverage.missing_skills.is_empty() {
        writeln!(out, "\nFoes using skills with no file:")?;
        for (foe, skills) in &coverage.missing_skills {
            let names: Vec<String> = skills.iter().map(|slug| slug.to_string()).collect();
            writeln!(out, "  {foe}: {}", names.join(", "))?;
        }
    }

    if !coverage.foes_with_unencoded_skills.is_empty() {
        writeln!(out, "\nFoes whose skills are seeded but not encoded:")?;
        for (foe, skills) in &coverage.foes_with_unencoded_skills {
            let names: Vec<String> = skills.iter().map(|slug| slug.to_string()).collect();
            writeln!(out, "  {foe}: {}", names.join(", "))?;
        }
    }

    Ok(())
}

fn write_row(name: &str, counts: &StatusCounts, out: &mut impl Write) -> io::Result<()> {
    writeln!(
        out,
        "{name:<16} {:>10} {:>7} {:>9} {:>7} {:>8.0}%",
        counts.numbers_only,
        counts.draft,
        counts.reviewed,
        counts.total(),
        counts.encoded_percent()
    )
}

fn as_json(coverage: &Coverage) -> serde_json::Value {
    let counts = |counts: &StatusCounts| {
        serde_json::json!({
            "numbers_only": counts.numbers_only,
            "draft": counts.draft,
            "reviewed": counts.reviewed,
            "total": counts.total(),
            "encoded_percent": counts.encoded_percent(),
            "reviewed_percent": counts.reviewed_percent(),
        })
    };

    let by_profession: serde_json::Map<String, serde_json::Value> = coverage
        .by_profession
        .iter()
        .map(|(profession, value)| {
            let name = profession
                .map(|profession| format!("{profession:?}"))
                .unwrap_or_else(|| "common".to_owned());
            (name, counts(value))
        })
        .collect();

    let by_campaign: serde_json::Map<String, serde_json::Value> = coverage
        .by_campaign
        .iter()
        .map(|(campaign, value)| (format!("{campaign:?}"), counts(value)))
        .collect();

    let listing =
        |map: &std::collections::BTreeMap<gwsim_data::ids::Slug, Vec<gwsim_data::ids::Slug>>| {
            let entries: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(foe, skills)| {
                    let names: Vec<String> = skills.iter().map(|slug| slug.to_string()).collect();
                    (foe.to_string(), serde_json::json!(names))
                })
                .collect();
            serde_json::Value::Object(entries)
        };

    serde_json::json!({
        "by_profession": by_profession,
        "by_campaign": by_campaign,
        "total": counts(&coverage.total),
        "missing_skills": listing(&coverage.missing_skills),
        "foes_with_unencoded_skills": listing(&coverage.foes_with_unencoded_skills),
        "denominator_is_complete": coverage.denominator_is_complete,
    })
}
