//! `gwsim data` — inspecting and checking the game data.

use std::io::{self, Write};

use gwsim_data::core::CoreData;
use gwsim_data::dataset::DataSet;
use gwsim_data::error::DataErrors;
use gwsim_data::source::DirSource;

use crate::{OutputFormat, ValidateArgs};

/// The exit code a command finished with.
///
/// Warnings alone leave this at zero: a tree that is usable but imperfect
/// should not fail a build.
pub const OK: i32 = 0;
/// Something in the data stops it being used.
pub const FAILED: i32 = 1;

/// Runs `gwsim data validate`.
///
/// Writes the report to `out` and returns the exit code, rather than exiting,
/// so tests can call it directly.
pub fn validate(args: &ValidateArgs, out: &mut impl Write) -> io::Result<i32> {
    let origin = crate::loading::choose(args.data_dir.as_deref());
    let problems = collect(&origin);

    match args.format {
        OutputFormat::Text => write_text(&problems, &origin, out)?,
        OutputFormat::Json => write_json(&problems, out)?,
    }

    Ok(if problems.is_fatal() { FAILED } else { OK })
}

/// Loads both halves of the data and gathers everything wrong with them.
///
/// The core files and the entity tree are checked by different loaders — the
/// core files are cross-checked against the enums in code — but an author
/// wants one report, so the two are merged here.
fn collect(origin: &crate::loading::DataOrigin) -> DataErrors {
    use crate::loading::DataOrigin;

    let mut problems = DataErrors::default();

    let data_dir = match origin {
        DataOrigin::Named(path) | DataOrigin::WorkingDirectory(path) => path.clone(),
        DataOrigin::Embedded => {
            // The embedded pack was validated at build time, so anything
            // wrong with it is a build problem. Checking it again costs
            // nothing and makes the command say something useful either way.
            match gwsim_data::pack::DataPack::from_bytes(crate::loading::embedded_bytes()) {
                Ok(pack) => problems.0.extend(pack.data.warnings().0.iter().cloned()),
                Err(pack_problems) => problems.0.extend(pack_problems.0),
            }
            problems.sort();
            return problems;
        }
    };

    if !data_dir.is_dir() {
        problems.0.push(gwsim_data::error::DataError::new(
            data_dir.display().to_string(),
            gwsim_data::error::DataErrorKind::Io,
            "this is not a directory; pass --data-dir to say where the data is",
        ));
        return problems;
    }

    if let Err(core_problems) = CoreData::load(data_dir.join("core")) {
        problems.0.extend(core_problems.into_data_errors("core"));
    }

    let source = DirSource::new(&data_dir);
    match DataSet::load(&source) {
        Ok(data) => problems.0.extend(data.warnings().0.iter().cloned()),
        Err(tree_problems) => problems.0.extend(tree_problems.0),
    }

    problems.sort();
    problems
}

fn write_text(
    problems: &DataErrors,
    origin: &crate::loading::DataOrigin,
    out: &mut impl Write,
) -> io::Result<()> {
    if problems.is_empty() {
        writeln!(out, "{origin}: no problems found.")?;
        return Ok(());
    }
    writeln!(out, "checked {origin}")?;
    writeln!(out)?;
    writeln!(out, "{problems}")?;
    Ok(())
}

fn write_json(problems: &DataErrors, out: &mut impl Write) -> io::Result<()> {
    let json = serde_json::to_string_pretty(&problems.0)
        .map_err(|error| io::Error::other(error.to_string()))?;
    writeln!(out, "{json}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OutputFormat;
    use std::path::PathBuf;

    fn repo_data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn args(dir: PathBuf, format: OutputFormat) -> ValidateArgs {
        ValidateArgs {
            data_dir: Some(dir),
            format,
        }
    }

    #[test]
    fn the_repositorys_own_data_validates() {
        let mut out = Vec::new();
        let code = validate(&args(repo_data_dir(), OutputFormat::Text), &mut out).unwrap();
        let report = String::from_utf8(out).unwrap();
        assert_eq!(code, OK, "data/ should validate, but:\n{report}");
    }

    #[test]
    fn a_missing_directory_says_how_to_point_at_one() {
        let mut out = Vec::new();
        let code = validate(
            &args(PathBuf::from("no/such/place"), OutputFormat::Text),
            &mut out,
        )
        .unwrap();
        assert_eq!(code, FAILED);

        let report = String::from_utf8(out).unwrap();
        assert!(report.contains("--data-dir"), "unhelpful report: {report}");
    }

    #[test]
    fn json_output_is_an_array_of_objects() {
        let mut out = Vec::new();
        validate(
            &args(PathBuf::from("no/such/place"), OutputFormat::Json),
            &mut out,
        )
        .unwrap();

        let report = String::from_utf8(out).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&report).expect("valid JSON");
        let array = parsed.as_array().expect("an array");
        assert_eq!(array.len(), 1);

        let problem = &array[0];
        assert_eq!(problem["kind"], "io");
        assert_eq!(problem["severity"], "error");
        assert!(problem["file"].is_string());
        assert!(problem["message"].is_string());
    }

    #[test]
    fn a_clean_run_says_so_rather_than_printing_nothing() {
        let mut out = Vec::new();
        validate(&args(repo_data_dir(), OutputFormat::Text), &mut out).unwrap();
        let report = String::from_utf8(out).unwrap();
        // Since P2 seeded real files the tree carries warnings (A-004 is
        // still Pending), so a passing run may end "0 errors, N warnings"
        // rather than "no problems found". Either way it must say so.
        assert!(
            report.contains("no problems found") || report.contains("0 errors"),
            "a silent success is indistinguishable from a broken command: {report}"
        );
    }
}
