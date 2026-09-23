//! `gwsim evaluate` — many seeded runs of one party in one situation
//! (T3.10.6).
//!
//! The minimal form: a party file (or a party in the data), a situation, and
//! either a fixed number of runs or the stop-when-stable rule. It prints the
//! win rate, clear time, and per-skill uses and damage. WP4.9 completes the
//! reports.

use std::io::{self, Write};
use std::path::Path;

use gwsim_data::dataset::DataSet;
use gwsim_data::party::PartyFile;
use gwsim_data::scenario::Situation;
use gwsim_engine::harness::{self, EvalOptions, Evaluation};
use gwsim_engine::log;
use gwsim_engine::{FightSetup, RunSeed, SeedList};

use crate::EvaluateArgs;
use crate::data::{FAILED, OK};

/// The seed used when none is given, so two runs of the command agree.
pub const DEFAULT_SEED: u64 = 1;

/// Runs `gwsim evaluate`.
pub fn run(args: &EvaluateArgs, out: &mut impl Write) -> io::Result<i32> {
    let loaded = match crate::loading::load(args.data_dir.as_deref()) {
        Ok(loaded) => loaded,
        Err((origin, problems)) => {
            writeln!(out, "could not read {origin}:")?;
            writeln!(out, "{problems}")?;
            return Ok(FAILED);
        }
    };
    let core = match crate::loading::load_core(&loaded.origin) {
        Ok(core) => core,
        Err(problem) => {
            writeln!(out, "could not read the core data: {problem}")?;
            return Ok(FAILED);
        }
    };
    let party = match find_party(&loaded.data, &args.party) {
        Ok(party) => party,
        Err(problem) => {
            writeln!(out, "{problem}")?;
            return Ok(FAILED);
        }
    };
    let situation = match find_situation(&loaded.data, &args.situation) {
        Ok(situation) => situation,
        Err(problem) => {
            writeln!(out, "{problem}")?;
            return Ok(FAILED);
        }
    };
    let setup = match FightSetup::new(&loaded.data, &core, &party, &situation) {
        Ok(setup) => setup,
        Err(error) => {
            writeln!(out, "the fight could not be prepared:\n{error}")?;
            return Ok(FAILED);
        }
    };

    let seed = args.seed.unwrap_or(DEFAULT_SEED);
    if let Some(format) = &args.log {
        let result = setup.run_logged(SeedList::new(seed, 1).get(0));
        let events = result.log.as_deref().unwrap_or_default();
        let text = match format.as_str() {
            "json" => log::to_json_lines(events),
            _ => log::to_text(
                events,
                |id| {
                    result
                        .unit_names
                        .get(usize::from(id))
                        .cloned()
                        .unwrap_or_else(|| setup.unit_name(id))
                },
                |id| setup.skill_name(id),
            ),
        };
        write!(out, "{text}")?;
        return Ok(OK);
    }

    let evaluate = || match args.runs {
        Some(runs) => harness::evaluate(&setup, &SeedList::new(seed, runs)),
        None => harness::evaluate_until_stable(&setup, seed, EvalOptions::default()),
    };
    let evaluation = match args.threads {
        Some(threads) => match rayon::ThreadPoolBuilder::new().num_threads(threads).build() {
            Ok(pool) => pool.install(evaluate),
            Err(error) => {
                writeln!(out, "could not start {threads} threads: {error}")?;
                return Ok(FAILED);
            }
        },
        None => evaluate(),
    };

    if args.json {
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&summary_json(&evaluation, &setup, seed))?
        )?;
    } else {
        write_text(
            out,
            &evaluation,
            &setup,
            &party,
            &situation,
            seed,
            &loaded.origin,
        )?;
    }
    Ok(OK)
}

/// A party by file path, or by slug in the data.
pub fn find_party(data: &DataSet, name: &str) -> Result<PartyFile, String> {
    let path = Path::new(name);
    if path.is_file() {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("could not read {name}: {e}"))?;
        return ron::from_str(&text).map_err(|e| format!("{name} is not a party file: {e}"));
    }
    let slug = name
        .parse()
        .map_err(|_| format!("{name:?} is neither a file nor a party slug"))?;
    data.party(&slug)
        .cloned()
        .ok_or_else(|| format!("no party {name:?} in the data"))
}

/// A situation by file path, or by slug in the data.
fn find_situation(data: &DataSet, name: &str) -> Result<Situation, String> {
    let path = Path::new(name);
    if path.is_file() {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("could not read {name}: {e}"))?;
        return ron::from_str(&text).map_err(|e| format!("{name} is not a situation file: {e}"));
    }
    let slug = name
        .parse()
        .map_err(|_| format!("{name:?} is neither a file nor a situation slug"))?;
    data.situations
        .get(&slug)
        .map(|entry| entry.value.clone())
        .ok_or_else(|| format!("no situation {name:?} in the data"))
}

/// Per-skill totals across every run, in bar order of first appearance.
fn skill_rows(evaluation: &Evaluation, setup: &FightSetup) -> Vec<(String, f64, f64)> {
    let runs = evaluation.runs.max(1) as f64;
    let Some(first) = evaluation.results.first() else {
        return Vec::new();
    };
    (0..first.stats.skills.len())
        .map(|index| {
            let id = first.stats.skills[index].id;
            let (uses, damage) = evaluation.results.iter().fold((0u64, 0i64), |(u, d), r| {
                let skill = &r.stats.skills[index];
                (u + u64::from(skill.uses), d + skill.damage)
            });
            (
                setup.skill_name(id),
                uses as f64 / runs,
                damage as f64 / runs,
            )
        })
        .collect()
}

fn summary_json(evaluation: &Evaluation, setup: &FightSetup, seed: u64) -> serde_json::Value {
    let mut assumptions: Vec<String> = evaluation
        .results
        .iter()
        .flat_map(|r| r.assumptions_touched.iter().map(|a| a.to_string()))
        .collect();
    assumptions.sort();
    assumptions.dedup();
    serde_json::json!({
        "seed": seed,
        "runs": evaluation.runs,
        "wins": evaluation.wins,
        "win_rate": evaluation.win_rate,
        "win_ci": evaluation.win_ci,
        "clear_time_s": evaluation.clear_time_s,
        "deaths": evaluation.deaths,
        "stop": evaluation.stop,
        "digests": evaluation.results.iter().map(|r| format!("{:016x}", r.digest())).collect::<Vec<_>>(),
        "skills": skill_rows(evaluation, setup)
            .into_iter()
            .map(|(name, uses, damage)| serde_json::json!({"skill": name, "uses_per_run": uses, "damage_per_run": damage}))
            .collect::<Vec<_>>(),
        "assumptions_touched": assumptions,
    })
}

fn write_text(
    out: &mut impl Write,
    evaluation: &Evaluation,
    setup: &FightSetup,
    party: &PartyFile,
    situation: &Situation,
    seed: u64,
    origin: &crate::loading::DataOrigin,
) -> io::Result<()> {
    writeln!(out, "{} in {}", party.name, situation.name)?;
    writeln!(out, "  data:       {origin}")?;
    writeln!(out, "  seed:       {seed}")?;
    writeln!(
        out,
        "  runs:       {} ({:?})",
        evaluation.runs, evaluation.stop
    )?;
    writeln!(
        out,
        "  win rate:   {:.1}%  (95% CI {:.1}% to {:.1}%)",
        evaluation.win_rate * 100.0,
        evaluation.win_ci.low * 100.0,
        evaluation.win_ci.high * 100.0
    )?;
    if evaluation.wins > 0 {
        let clear = &evaluation.clear_time_s;
        writeln!(
            out,
            "  clear time: {:.1} s mean, {:.1} s median  (95% CI {:.1} to {:.1} s)",
            clear.mean, clear.median, clear.ci.low, clear.ci.high
        )?;
    } else {
        writeln!(out, "  clear time: no wins")?;
    }
    writeln!(out, "  deaths:     {:.2} per run", evaluation.deaths.mean)?;
    writeln!(out)?;
    writeln!(
        out,
        "  {:<24} {:>10} {:>14}",
        "skill", "uses/run", "damage/run"
    )?;
    for (name, uses, damage) in skill_rows(evaluation, setup) {
        writeln!(out, "  {name:<24} {uses:>10.2} {damage:>14.1}")?;
    }
    let mut assumptions: Vec<String> = evaluation
        .results
        .iter()
        .flat_map(|r| r.assumptions_touched.iter().map(|a| a.to_string()))
        .collect();
    assumptions.sort();
    assumptions.dedup();
    if !assumptions.is_empty() {
        writeln!(out)?;
        writeln!(out, "  assumptions touched: {}", assumptions.join(", "))?;
    }
    Ok(())
}

/// The seed a single run used, for tests.
pub fn first_seed(master: u64) -> RunSeed {
    SeedList::new(master, 1).get(0)
}
