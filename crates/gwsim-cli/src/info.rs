//! `gwsim data info` — which data this binary is using (T1.7.6).

use std::io::{self, Write};

use gwsim_data::user_dir::UserDir;

use crate::InfoArgs;
use crate::data::{FAILED, OK};

/// Runs `gwsim data info`.
pub fn run(args: &InfoArgs, out: &mut impl Write) -> io::Result<i32> {
    let loaded = match crate::loading::load(args.data_dir.as_deref()) {
        Ok(loaded) => loaded,
        Err((origin, problems)) => {
            writeln!(out, "could not read {origin}:")?;
            writeln!(out, "{problems}")?;
            return Ok(FAILED);
        }
    };

    let counts = loaded.data.counts();
    let user = UserDir::resolve(args.user_dir.as_deref());

    if args.json {
        let value = serde_json::json!({
            "gwsim_version": env!("CARGO_PKG_VERSION"),
            "data_source": loaded.origin.to_string(),
            "content_hash": loaded.version.content_hash,
            "short_hash": loaded.version.short_hash(),
            "baseline": loaded.version.baseline,
            "embedded_bytes": crate::loading::embedded_size(),
            "counts": {
                "skills": counts.skills,
                "foes": counts.foes,
                "encounters": counts.encounters,
                "situations": counts.situations,
                "situation_sets": counts.situation_sets,
                "benchmarks": counts.benchmarks,
                "assumptions": counts.assumptions,
            },
            "user_dir": user.as_ref().map(|dir| dir.root().display().to_string()),
            "user_dir_exists": user.as_ref().is_some_and(|dir| dir.exists()),
        });
        writeln!(out, "{}", serde_json::to_string_pretty(&value)?)?;
        return Ok(OK);
    }

    writeln!(out, "gwsim {}", env!("CARGO_PKG_VERSION"))?;
    writeln!(out, "  data source:  {}", loaded.origin)?;
    writeln!(out, "  content hash: {}", loaded.version.content_hash)?;
    writeln!(
        out,
        "  baseline:     {} (the game state these numbers describe)",
        loaded.version.baseline
    )?;
    writeln!(
        out,
        "  embedded:     {} bytes",
        crate::loading::embedded_size()
    )?;

    writeln!(out, "  contents:")?;
    for (label, count) in [
        ("skills", counts.skills),
        ("foes", counts.foes),
        ("encounters", counts.encounters),
        ("situations", counts.situations),
        ("situation sets", counts.situation_sets),
        ("benchmarks", counts.benchmarks),
        ("assumptions", counts.assumptions),
    ] {
        writeln!(out, "    {label:<16} {count}")?;
    }

    match &user {
        Some(dir) => {
            writeln!(out, "  your files:   {}", dir.root().display())?;
            if !dir.exists() {
                // Saying this matters: an absent directory is normal, not a
                // problem, and nothing is created until something is saved.
                writeln!(
                    out,
                    "                (not created yet; it appears when you first save \
                     something)"
                )?;
            }
        }
        None => writeln!(
            out,
            "  your files:   nowhere; set {} to choose a location",
            gwsim_data::user_dir::OVERRIDE_VAR
        )?,
    }

    Ok(OK)
}
