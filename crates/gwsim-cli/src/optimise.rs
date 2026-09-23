//! `gwsim optimise` — the build optimiser on the command line, with report 3
//! (WP5.8, §14 #1 and #3, §15).
//!
//! It frees the named slots of a party, searches them against a situation or
//! a weighted situation set (evolutionary by default, exhaustive over a pool
//! file on request), and prints the ranked builds with their template codes
//! and metrics, and the trade-off frontier. `--json` writes report 3, a
//! schema-versioned `OptimisationResult`. The time budget is measured here:
//! the optimiser crate reads no clock. Ctrl-C stops the search cleanly and
//! the current frontier is written, marked "interrupted".

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gwsim_data::build::Build;
use gwsim_data::party::PartyFile;
use gwsim_data::profile::AccountProfile;
use gwsim_data::template::SkillTemplate;
use gwsim_data::user_dir::UserDir;
use gwsim_engine::SeedList;
use gwsim_engine::harness::{self, Evaluation};
use gwsim_opt::evaluate::{EngineEvaluator, Progress, Stages};
use gwsim_opt::exhaustive::{self, PoolFile};
use gwsim_opt::genome::{LockMask, PartyGenome, SlotGenome};
use gwsim_opt::objectives::{Goal, Objective, Scoring, ThresholdRule};
use gwsim_opt::operators::SlotContext;
use gwsim_opt::pools::{PoolOptions, SlotPools};
use gwsim_opt::roles::{self, SlotRole};
use gwsim_opt::search::{
    self, CancelToken, Candidate, Config, Controls, Origin, Outcome, Problem, Seed,
};
use serde::Serialize;

use crate::OptimiseArgs;
use crate::data::{FAILED, OK};
use crate::evaluate::{self, Context, DEFAULT_SEED, Failure};
use crate::report;
use crate::results::{self, BuildReport, Notes, PackInfo, SituationInput};

/// Runs per situation used to collect the report notes for the winner.
const NOTE_RUNS: usize = 8;

/// Parses `5m`, `90s`, `1h` or a number of seconds.
pub fn parse_budget(text: &str) -> Option<Duration> {
    let text = text.trim();
    let (number, unit) = match text.char_indices().find(|(_, c)| c.is_ascii_alphabetic()) {
        Some((i, _)) => (&text[..i], &text[i..]),
        None => (text, "s"),
    };
    let value: f64 = number.trim().parse().ok()?;
    let seconds = match unit {
        "s" | "sec" => value,
        "m" | "min" => value * 60.0,
        "h" => value * 3600.0,
        _ => return None,
    };
    (seconds >= 0.0).then(|| Duration::from_secs_f64(seconds))
}

/// Parses `--lock player=1,2,gear,attributes,secondary`.
fn parse_lock(text: &str) -> Result<(String, LockMask), String> {
    let (slot, parts) = text
        .split_once('=')
        .ok_or_else(|| format!("--lock {text:?}: use slot=positions,gear,attributes,secondary"))?;
    let mut mask = LockMask::default();
    for part in parts.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part {
            "gear" => mask.gear = true,
            "attributes" => mask.attributes = true,
            "secondary" => mask.secondary = true,
            number => {
                let position: usize = number
                    .parse()
                    .ok()
                    .filter(|p| (1..=8).contains(p))
                    .ok_or_else(|| {
                        format!("--lock {text:?}: {number:?} is not a bar position 1 to 8")
                    })?;
                mask.skills |= 1 << (position - 1);
            }
        }
    }
    Ok((slot.to_owned(), mask))
}

/// Report 3: what `--json` writes.
#[derive(Debug, Serialize)]
pub struct OptimisationResult {
    pub schema_version: u32,
    pub interrupted: bool,
    pub inputs: OptimiseInputs,
    pub outcome: Outcome,
    /// Report 1 for each ranked candidate's free slots, in ranked order.
    pub ranked_builds: Vec<Vec<BuildReport>>,
    pub notes: Notes,
}

/// The inputs of a search, for the record.
#[derive(Debug, Serialize)]
pub struct OptimiseInputs {
    pub party: PartyFile,
    pub free: Vec<String>,
    pub situations: Vec<SituationInput>,
    pub set: Option<String>,
    pub mode: String,
    pub config: Config,
    pub budget_s: Option<f64>,
    pub profile: String,
    pub reviewed_only: bool,
    pub pack: PackInfo,
}

/// Runs `gwsim optimise`.
pub fn run(args: &OptimiseArgs, out: &mut impl Write) -> io::Result<i32> {
    match optimise(args, out) {
        Ok(code) => Ok(code),
        Err(Failure::Io(error)) => Err(error),
        Err(Failure::Message(message)) => {
            writeln!(out, "{message}")?;
            Ok(FAILED)
        }
    }
}

/// Loads a profile by name from the user directory, or the default.
pub fn load_profile(name: Option<&str>, user_dir: Option<&Path>) -> Result<AccountProfile, String> {
    let Some(name) = name else {
        return Ok(AccountProfile::default());
    };
    let dir = UserDir::resolve(user_dir).ok_or("no user directory: pass --user-dir")?;
    AccountProfile::read(&AccountProfile::path_in(&dir.subfolder("profiles"), name))
}

fn optimise(args: &OptimiseArgs, out: &mut impl Write) -> Result<i32, Failure> {
    let context = evaluate::load_context(args.data_dir.as_deref())?;
    let data = &context.loaded.data;
    let party = evaluate::find_party(data, &args.party)?;
    let is_set = args
        .situations
        .parse()
        .ok()
        .is_some_and(|slug| data.situation_sets.contains_key(&slug));
    let (inputs, set) = if is_set {
        evaluate::situation_inputs(data, None, Some(&args.situations))?
    } else {
        evaluate::situation_inputs(data, Some(&args.situations), None)?
    };
    for input in &inputs {
        let size = usize::from(input.situation.party_size);
        if size != 0 && size != party.slots.len() {
            return Err(Failure::Message(format!(
                "{} is for a party of {size}, but {} has {} slots (§12.5)",
                input.situation.name,
                party.name,
                party.slots.len()
            )));
        }
    }

    // Free slots and locks.
    if args.free.is_empty() {
        return Err(Failure::Message(
            "name at least one slot with --free".to_owned(),
        ));
    }
    let mut locks: BTreeMap<String, LockMask> = BTreeMap::new();
    for text in &args.lock {
        let (slot, mask) = parse_lock(text)?;
        locks.insert(slot, mask);
    }
    let mut free = Vec::new();
    for name in &args.free {
        let index = party
            .slots
            .iter()
            .position(|s| s.name == *name)
            .ok_or_else(|| format!("--free {name:?}: the party has no such slot"))?;
        free.push((index, locks.get(name).copied().unwrap_or_default()));
    }

    // Profile, pools and roles.
    let profile = load_profile(args.profile.as_deref(), args.user_dir.as_deref())?;
    let accord = inputs.iter().any(|i| i.situation.mode.melandrus_accord);
    let options = PoolOptions {
        reviewed_only: args.reviewed_only,
        profile: profile.clone(),
        accord,
    };
    let mut role_overrides: BTreeMap<String, SlotRole> = BTreeMap::new();
    for text in &args.role {
        let (slot, role) = text
            .split_once('=')
            .ok_or_else(|| format!("--role {text:?}: use slot=role"))?;
        let role = SlotRole::parse(role).ok_or_else(|| {
            format!("--role {text:?}: roles are healer, protection, damage, interrupt, minions, spirits, support")
        })?;
        role_overrides.insert(slot.to_owned(), role);
    }
    let mut contexts = BTreeMap::new();
    for (index, _) in &free {
        let slot = &party.slots[*index];
        if let Some(hero) = &slot.hero
            && !profile.heroes_owned.allows(hero)
        {
            return Err(Failure::Message(format!(
                "{}: the profile does not own the hero {hero}",
                slot.name
            )));
        }
        let role = role_overrides
            .get(&slot.name)
            .copied()
            .unwrap_or_else(|| SlotRole::infer(&slot.build, data));
        contexts.insert(
            *index,
            SlotContext {
                pools: SlotPools::for_slot(slot, data, &options),
                role,
            },
        );
    }

    let seeds = seeds_for(&context, &party, &free, &contexts);
    let problem = Problem {
        data,
        base: party.clone(),
        free: free.clone(),
        contexts,
        seeds,
    };

    // Scoring and settings.
    let objectives: Vec<Objective> = match &args.objectives {
        Some(text) => text
            .split(',')
            .map(|o| Objective::parse(o.trim()).ok_or_else(|| format!("unknown objective {o:?}")))
            .collect::<Result<_, _>>()?,
        None => vec![Objective::ClearTime, Objective::Deaths],
    };
    let goal = match &args.goal {
        Some(text) => Goal::parse(text).ok_or_else(|| format!("--goal {text:?} is not a goal"))?,
        None => Goal::default(),
    };
    let mut objectives = objectives;
    for (objective, _) in &goal.terms {
        if !objectives.contains(objective) {
            objectives.push(*objective);
        }
    }
    let scoring = Scoring {
        objectives,
        goal,
        threshold: args.threshold,
        rule: if args.aggregate_threshold {
            ThresholdRule::WeightedAggregate
        } else {
            ThresholdRule::EverySituation
        },
        weights: inputs.iter().map(|i| i.weight).collect(),
    };
    let seed = args.seed.unwrap_or(DEFAULT_SEED);
    let mut config = Config::new(scoring, seed);
    config.population = args.population;
    config.max_generations = args.generations;
    if let Some(stages) = &args.stages {
        let numbers: Vec<usize> = stages
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if numbers.len() != 3
            || numbers[0] == 0
            || numbers[0] > numbers[1]
            || numbers[1] > numbers[2]
        {
            return Err(Failure::Message(
                "--stages takes three rising run counts, like 16,64,256".to_owned(),
            ));
        }
        config.stages = Stages {
            first: numbers[0],
            second: numbers[1],
            frontier: numbers[2],
        };
    }
    config.report = args.top.max(1);
    let budget = match &args.budget {
        Some(text) => Some(
            parse_budget(text).ok_or_else(|| format!("--budget {text:?}: use 90s, 5m or 1h"))?,
        ),
        None if args.generations.is_none() => Some(Duration::from_secs(300)),
        None => None,
    };

    let evaluator = EngineEvaluator {
        data,
        core: &context.core,
        situations: inputs.iter().map(|i| i.situation.clone()).collect(),
        title_ranks: profile.title_ranks.clone(),
    };

    let cancel = CancelToken::new();
    {
        let token = cancel.clone();
        // A second handler cannot be installed in one process; tests call
        // this repeatedly, so a failure here just means one exists.
        let _ = ctrlc::set_handler(move || token.cancel());
    }
    let started = Instant::now();
    let should_stop = || budget.is_some_and(|b| started.elapsed() >= b);
    let quiet = args.quiet;
    let mut on_snapshot = |snapshot: &search::Snapshot| {
        if quiet {
            return;
        }
        let best = snapshot
            .best
            .as_ref()
            .map(|b| format!("best {:.2}", b.score.goal))
            .unwrap_or_else(|| "no feasible build yet".to_owned());
        eprintln!(
            "generation {}: {} candidates, {} runs, frontier {}, {best}, {:.0} s",
            snapshot.generation,
            snapshot.candidates,
            snapshot.runs_simulated,
            snapshot.frontier.len(),
            started.elapsed().as_secs_f64()
        );
    };
    let on_progress = |_: Progress| {};
    let controls = Controls {
        should_stop: &should_stop,
        cancel: cancel.clone(),
        on_snapshot: &mut on_snapshot,
        on_progress: &on_progress,
    };

    let mode = args.mode.as_str();
    let outcome = evaluate::with_threads(args.threads, || match mode {
        "exhaustive" => {
            let Some(path) = &args.pool else {
                return Err("exhaustive mode needs --pool <file>".to_owned());
            };
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("could not read {}: {e}", path.display()))?;
            let pool: PoolFile = ron::from_str(&text)
                .map_err(|e| format!("{} is not a pool file: {e}", path.display()))?;
            exhaustive::run(
                &problem,
                &evaluator,
                config.clone(),
                &pool,
                args.limit,
                controls,
            )
            .map_err(|refusal| refusal.to_string())
        }
        "evolutionary" => search::optimise(&problem, &evaluator, config.clone(), controls),
        other => Err(format!(
            "unknown mode {other:?}: use evolutionary or exhaustive"
        )),
    })??;

    // Notes from a short evaluation of the best build (or the party as given).
    let winner_party = outcome
        .ranked
        .first()
        .map(|c| party_with(&party, c))
        .unwrap_or_else(|| party.clone());
    let setups = evaluate::prepare(&context, &winner_party, &inputs)?;
    let evaluations: Vec<Evaluation> = setups
        .iter()
        .map(|s| harness::evaluate(s, &SeedList::new(seed, NOTE_RUNS)))
        .collect();
    let setup_refs: Vec<&gwsim_engine::FightSetup> = setups.iter().collect();
    let mut notes = Notes::collect(
        data,
        PackInfo::of(&context.loaded),
        seed,
        &[&party],
        &evaluations,
        &setup_refs,
    );
    notes.runs = vec![config.stages.frontier; inputs.len()];

    let ranked_builds: Vec<Vec<BuildReport>> = outcome
        .ranked
        .iter()
        .map(|c| {
            let candidate_party = party_with(&party, c);
            c.builds
                .iter()
                .filter_map(|b| candidate_party.slots.iter().find(|s| s.name == b.slot))
                .map(|slot| results::build_report(slot, data))
                .collect()
        })
        .collect();
    let result = OptimisationResult {
        schema_version: results::SCHEMA_VERSION,
        interrupted: outcome.interrupted(),
        inputs: OptimiseInputs {
            party: party.clone(),
            free: args.free.clone(),
            situations: inputs.clone(),
            set,
            mode: mode.to_owned(),
            config: config.clone(),
            budget_s: budget.map(|b| b.as_secs_f64()),
            profile: profile.name.clone(),
            reviewed_only: args.reviewed_only,
            pack: PackInfo::of(&context.loaded),
        },
        outcome,
        ranked_builds,
        notes,
    };

    if let Some(path) = &args.json {
        let text = serde_json::to_string_pretty(&result).map_err(io::Error::other)?;
        std::fs::write(path, text)
            .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    }
    write_text(out, &result, &inputs)?;
    if let Some(path) = &args.json {
        writeln!(out, "result written to {}", path.display())?;
    }
    Ok(OK)
}

/// The party with a candidate's free-slot builds in place.
pub fn party_with(base: &PartyFile, candidate: &Candidate) -> PartyFile {
    let mut party = base.clone();
    for build in &candidate.builds {
        if let Some(slot) = party.slots.iter_mut().find(|s| s.name == build.slot) {
            if slot.build.skills != build.build.skills {
                slot.plan = None;
            }
            slot.build = build.build.clone();
        }
    }
    party
}

/// Generation 0's seeds (T5.2.6): benchmark bars that fit a free slot's
/// primary, and a heuristic bar per role for each free slot.
fn seeds_for(
    context: &Context,
    party: &PartyFile,
    free: &[(usize, LockMask)],
    contexts: &BTreeMap<usize, SlotContext>,
) -> Vec<Seed> {
    let data = &context.loaded.data;
    let mut seeds = Vec::new();
    let base = PartyGenome::from_party(party, free);
    for (index, _) in free {
        let slot = &party.slots[*index];
        let Some(ctx) = contexts.get(index) else {
            continue;
        };
        // Benchmark bars with this slot's primary, on the slot's own gear.
        for benchmark in data.benchmarks.values() {
            for (n, bench) in benchmark.value.slots.iter().enumerate() {
                let code = bench.bar_code.as_deref().unwrap_or(&bench.skill_code);
                let Ok(template) = SkillTemplate::decode(code) else {
                    continue;
                };
                if template.primary != slot.build.primary {
                    continue;
                }
                let Ok((from_code, _)) = Build::from_templates(&template, None, data) else {
                    continue;
                };
                let mut genome = base.clone();
                if let SlotGenome::Free(f) = &mut genome.slots[*index] {
                    f.build.secondary = from_code.secondary;
                    f.build.skills = from_code.skills;
                    f.build.attribute_points = from_code.attribute_points;
                }
                seeds.push(Seed {
                    genome,
                    origin: Origin::Benchmark(format!("{} slot {}", benchmark.value.name, n + 1)),
                });
            }
        }
        // A heuristic bar per role.
        for role in SlotRole::ALL {
            let candidates = ctx
                .pools
                .skills_for(data, slot.build.primary, slot.build.secondary);
            let bar = roles::heuristic_bar(
                role,
                slot.build.primary,
                slot.build.secondary,
                &candidates,
                data,
            );
            let mut genome = base.clone();
            if let SlotGenome::Free(f) = &mut genome.slots[*index] {
                f.build.skills = bar.skills;
                f.build.attribute_points = bar.attribute_points;
            }
            seeds.push(Seed {
                genome,
                origin: Origin::Heuristic(format!("{} {role:?}", slot.name)),
            });
        }
    }
    seeds
}

fn origin_label(origin: &Origin) -> String {
    match origin {
        Origin::Current => "the party as given".to_owned(),
        Origin::Benchmark(name) => format!("benchmark ({name})"),
        Origin::Heuristic(name) => format!("heuristic ({name})"),
        Origin::Random => "random".to_owned(),
        Origin::Generation(g) => format!("generation {g}"),
        Origin::Exhaustive(n) => format!("combination {n}"),
    }
}

fn write_text(
    out: &mut impl Write,
    result: &OptimisationResult,
    inputs: &[SituationInput],
) -> io::Result<()> {
    let outcome = &result.outcome;
    report::write_header(
        out,
        &format!(
            "gwsim optimise: {} (free: {})",
            result.inputs.party.name,
            result.inputs.free.join(", ")
        ),
        &result.notes,
    )?;
    writeln!(
        out,
        "Search: {} mode, {} generations, {} candidates, {} runs, stopped by {:?}{}",
        result.inputs.mode,
        outcome.generations,
        outcome.candidates,
        outcome.runs_simulated,
        outcome.stop,
        if result.interrupted {
            " (interrupted)"
        } else {
            ""
        }
    )?;
    let objectives = &result.inputs.config.scoring.objectives;
    writeln!(
        out,
        "Goal: {}; threshold {:.0}% ({:?})",
        result
            .inputs
            .config
            .scoring
            .goal
            .terms
            .iter()
            .map(|(o, w)| format!("{o:?}×{w}"))
            .collect::<Vec<_>>()
            .join(" + "),
        result.inputs.config.scoring.threshold * 100.0,
        result.inputs.config.scoring.rule
    )?;
    writeln!(out)?;

    writeln!(out, "Ranked builds")?;
    for (rank, (candidate, builds)) in outcome.ranked.iter().zip(&result.ranked_builds).enumerate()
    {
        writeln!(
            out,
            "{:>2}. {} — goal {:.2}, {}, {} runs per situation",
            rank + 1,
            origin_label(&candidate.origin),
            candidate.score.goal,
            if candidate.score.feasible {
                "meets the threshold".to_owned()
            } else {
                format!(
                    "short of the threshold by {:.1} points",
                    candidate.score.violation * 100.0
                )
            },
            candidate.runs
        )?;
        for build in builds {
            let skills: Vec<&str> = build
                .skills
                .iter()
                .map(|s| s.as_deref().unwrap_or("(empty)"))
                .collect();
            writeln!(
                out,
                "    {} ({}): {}",
                build.slot,
                build.professions,
                skills.join(", ")
            )?;
            writeln!(out, "      skill code {}", build.skill_code)?;
            if let Some(code) = &build.equipment_code {
                writeln!(out, "      equipment code {code}")?;
            }
        }
        for (stats, input) in candidate.score.situations.iter().zip(inputs) {
            writeln!(
                out,
                "      {:<28} win {:>5.1}% ({:.1}–{:.1}), clear {}, deaths {:.2}, damage {:.0}, energy {:.0}",
                input.situation.name,
                stats.win_rate * 100.0,
                stats.win_low * 100.0,
                stats.win_high * 100.0,
                stats
                    .clear_time_s
                    .map(|c| format!("{c:.1} s"))
                    .unwrap_or_else(|| "none".to_owned()),
                stats.deaths,
                stats.damage_taken,
                stats.energy_left
            )?;
        }
    }
    writeln!(out)?;
    writeln!(out, "Frontier ({} builds)", outcome.frontier.len())?;
    let header: Vec<String> = objectives.iter().map(|o| format!("{o:?}")).collect();
    writeln!(
        out,
        "    {:<34} {}",
        "build",
        header
            .iter()
            .map(|h| format!("{h:>12}"))
            .collect::<String>()
    )?;
    for candidate in &outcome.frontier {
        let values: String = candidate
            .score
            .objectives
            .iter()
            .zip(objectives)
            .map(|(v, o)| format!("{:>12.2}", o.display(*v)))
            .collect();
        let label = candidate
            .builds
            .iter()
            .map(|b| {
                SkillTemplate {
                    primary: b.build.primary,
                    secondary: b.build.secondary,
                    attributes: b
                        .build
                        .attribute_points
                        .iter()
                        .map(|(a, r)| (*a, *r))
                        .collect(),
                    skills: b.build.skills,
                }
                .encode()
            })
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(out, "    {label:<34} {values}")?;
    }
    writeln!(out)?;
    report::write_footer(out, &result.notes)
}

/// Where a `--json` path would go by default in the user directory.
pub fn default_result_path(dir: &UserDir, name: &str) -> PathBuf {
    dir.subfolder("results").join(format!("{name}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_parse() {
        assert_eq!(parse_budget("5m"), Some(Duration::from_secs(300)));
        assert_eq!(parse_budget("90s"), Some(Duration::from_secs(90)));
        assert_eq!(parse_budget("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_budget("30"), Some(Duration::from_secs(30)));
        assert_eq!(parse_budget("soon"), None);
    }

    #[test]
    fn locks_parse() {
        let (slot, mask) = parse_lock("player=1,3,gear").unwrap();
        assert_eq!(slot, "player");
        assert_eq!(mask.skills, 0b101);
        assert!(mask.gear && !mask.attributes);
        assert!(parse_lock("player=9").is_err());
    }
}
