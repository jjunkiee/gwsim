//! `gwsim log` — report 5: one run's combat log, re-simulated from a result
//! file (T4.9.6).
//!
//! A result file holds the party, the situations and every run's seed, so
//! any run can be played again exactly. **It refuses when the data pack has
//! changed** since the result was written (ENG-43), because the same seed on
//! different data is a different fight, and a log that silently disagreed
//! with its summary would be worse than none. `--force` overrides that, with
//! a warning.

use std::io::{self, Write};

use gwsim_engine::log::{self, LogEvent, LogKind};
use gwsim_engine::RunSeed;
use gwsim_engine::harness::{Evaluation, StopReason};

use crate::LogArgs;
use crate::data::{FAILED, OK};
use crate::evaluate::{self, Failure};
use crate::report;
use crate::results::{Notes, PackInfo, RunSummary};

/// Log kinds shown only with `--verbose`: decisions, movement, energy ticks
/// and swing starts, which outnumber everything else.
fn is_verbose(kind: LogKind) -> bool {
    matches!(
        kind,
        LogKind::Decision
            | LogKind::MovementStarted
            | LogKind::MovementStopped
            | LogKind::EnergyChanged
            | LogKind::AttackStarted
    )
}

/// Runs `gwsim log`.
pub fn run(args: &LogArgs, out: &mut impl Write) -> io::Result<i32> {
    match print_log(args, out) {
        Ok(code) => Ok(code),
        Err(Failure::Io(error)) => Err(error),
        Err(Failure::Message(message)) => {
            writeln!(out, "{message}")?;
            Ok(FAILED)
        }
    }
}

fn print_log(args: &LogArgs, out: &mut impl Write) -> Result<i32, Failure> {
    let file = evaluate::read_result(&args.result)?;
    let context = evaluate::load_context(args.data_dir.as_deref())?;
    let pack = PackInfo::of(&context.loaded);
    if pack.content_hash != file.inputs.pack.content_hash {
        let message = format!(
            "the data pack has changed since this result was written (result {}, now {}), \
             so run {} would not be the same fight",
            file.inputs.pack.short_hash(),
            pack.short_hash(),
            args.run
        );
        if !args.force {
            return Err(Failure::Message(format!("{message}; pass --force to log it anyway")));
        }
        writeln!(out, "warning: {message}")?;
    }

    // Runs are numbered across situations in order, so run 70 of a set
    // whose first situation took 64 runs is the second situation's run 6.
    let mut index = args.run;
    let mut found = None;
    for (situation, report) in file.inputs.situations.iter().zip(&file.situations) {
        if index < report.runs.len() {
            found = Some((situation, &report.runs[index]));
            break;
        }
        index -= report.runs.len();
    }
    let total: usize = file.situations.iter().map(|s| s.runs.len()).sum();
    let Some((input, summary)) = found else {
        return Err(Failure::Message(format!(
            "there is no run {}: the result holds runs 0 to {}",
            args.run,
            total.saturating_sub(1)
        )));
    };

    let setups = evaluate::prepare(&context, &file.inputs.party, std::slice::from_ref(input))?;
    let setup = &setups[0];
    let result = setup.run_logged(RunSeed(summary.seed));
    let replayed = RunSummary::of(summary.index, &result);
    let events: Vec<LogEvent> = result
        .log
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|e| args.verbose || !is_verbose(e.kind))
        .cloned()
        .collect();

    match args.format.as_str() {
        "jsonl" => {
            write!(out, "{}", log::to_json_lines(&events))?;
        }
        "text" => {
            let mut stripped = result.clone();
            stripped.log = None;
            let evaluation = Evaluation::from_results(vec![stripped], StopReason::Fixed);
            let notes = Notes::collect(
                &context.loaded.data,
                pack,
                file.inputs.seed,
                &[&file.inputs.party],
                [&evaluation],
                &[setup],
            );
            report::write_header(
                out,
                &format!(
                    "gwsim log: run {} of {} in {} (seed {:016x})",
                    args.run, file.inputs.party.name, input.situation.name, summary.seed
                ),
                &notes,
            )?;
            let text = log::to_text(
                &events,
                |id| {
                    result
                        .unit_names
                        .get(usize::from(id))
                        .cloned()
                        .unwrap_or_else(|| setup.unit_name(id))
                },
                |id| setup.skill_name(id),
            );
            write!(out, "{text}")?;
            writeln!(out)?;
            writeln!(
                out,
                "outcome {} after {:.1} s, {} death(s)",
                replayed.outcome,
                f64::from(replayed.ended_ms) / 1000.0,
                replayed.deaths
            )?;
            report::write_footer(out, &notes)?;
        }
        other => {
            return Err(Failure::Message(format!(
                "unknown format {other:?}: use text or jsonl"
            )));
        }
    }

    if replayed != *summary {
        writeln!(
            out,
            "warning: the replay does not match the recorded summary (digest {} now, {} then)",
            replayed.digest, summary.digest
        )?;
        return Ok(FAILED);
    }
    Ok(OK)
}
