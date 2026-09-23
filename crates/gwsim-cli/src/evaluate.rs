//! `gwsim evaluate` — many seeded runs of one party in one situation or a
//! situation set (T3.10.6, T4.9.2).
//!
//! It prints report 1 (the builds and their codes) and report 2 (the metrics
//! with 95% intervals) for every situation, report 4 (contributions and the
//! energy timeline) with `--breakdown`, and the §14.1 notes around them.
//! `--json` writes the whole result file (§14.3, `docs/result-schema.md`).

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use gwsim_data::build::{Build, SlotKind};
use gwsim_data::core::CoreData;
use gwsim_data::dataset::DataSet;
use gwsim_data::party::{PartyFile, PartySlot};
use gwsim_data::scenario::Situation;
use gwsim_data::template::SkillTemplate;
use gwsim_engine::harness::{self, EvalOptions, Evaluation};
use gwsim_engine::log;
use gwsim_engine::{FightSetup, RunSeed, SeedList};

use crate::EvaluateArgs;
use crate::data::{FAILED, OK};
use crate::loading::Loaded;
use crate::report;
use crate::results::{self, Inputs, Notes, PackInfo, ResultFile, RunsRequest, SituationInput};

/// The seed used when none is given, so two runs of the command agree.
pub const DEFAULT_SEED: u64 = 1;

/// The most slots a party has.
pub const PARTY_SIZE: usize = 8;

/// Loaded data and core data, from wherever the command was pointed.
pub struct Context {
    pub loaded: Loaded,
    pub core: CoreData,
}

/// Loads the data and core data, or says why not.
pub fn load_context(data_dir: Option<&Path>) -> Result<Context, String> {
    let loaded = crate::loading::load(data_dir)
        .map_err(|(origin, problems)| format!("could not read {origin}:\n{problems}"))?;
    let core = crate::loading::load_core(&loaded.origin)
        .map_err(|problem| format!("could not read the core data: {problem}"))?;
    Ok(Context { loaded, core })
}

/// Runs `gwsim evaluate`.
pub fn run(args: &EvaluateArgs, out: &mut impl Write) -> io::Result<i32> {
    match evaluate(args, out) {
        Ok(code) => Ok(code),
        Err(Failure::Io(error)) => Err(error),
        Err(Failure::Message(message)) => {
            writeln!(out, "{message}")?;
            Ok(FAILED)
        }
    }
}

/// Why a command stopped.
pub enum Failure {
    Io(io::Error),
    Message(String),
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Failure::Io(error)
    }
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure::Message(message)
    }
}

fn evaluate(args: &EvaluateArgs, out: &mut impl Write) -> Result<i32, Failure> {
    let context = load_context(args.data_dir.as_deref())?;
    let data = &context.loaded.data;
    let party = find_party(data, &args.party)?;
    if args.reviewed_only {
        let unreviewed = results::unreviewed_skills(&party, data);
        if !unreviewed.is_empty() {
            return Err(Failure::Message(format!(
                "--reviewed-only: {} skill(s) on these bars are not Reviewed:\n  {}",
                unreviewed.len(),
                unreviewed.join("\n  ")
            )));
        }
    }
    let (inputs, set) = situation_inputs(data, args.situation.as_deref(), args.set.as_deref())?;
    let setups = prepare(&context, &party, &inputs)?;
    let seed = args.seed.unwrap_or(DEFAULT_SEED);

    if let Some(format) = &args.log {
        let setup = &setups[0];
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

    let request = match args.runs {
        Some(runs) => RunsRequest::Fixed(runs),
        None => RunsRequest::Auto,
    };
    let evaluations = with_threads(args.threads, || {
        setups
            .iter()
            .map(|setup| run_request(setup, request, seed))
            .collect::<Vec<_>>()
    })?;

    let file = result_file(
        &context,
        party,
        inputs,
        set,
        seed,
        request,
        args.reviewed_only,
        &setups,
        &evaluations,
    );

    match args.json.as_deref() {
        Some(path) if path == Path::new("-") => {
            writeln!(out, "{}", serde_json::to_string_pretty(&file).map_err(io::Error::other)?)?;
            return Ok(OK);
        }
        Some(path) => {
            let text = serde_json::to_string_pretty(&file).map_err(io::Error::other)?;
            std::fs::write(path, text)
                .map_err(|e| format!("could not write {}: {e}", path.display()))?;
        }
        None => {}
    }
    write_text(out, &file, args.breakdown)?;
    if let Some(path) = args.json.as_deref() {
        writeln!(out, "result written to {}", path.display())?;
    }
    Ok(OK)
}

/// Evaluates one setup as asked.
pub fn run_request(setup: &FightSetup, request: RunsRequest, seed: u64) -> Evaluation {
    match request {
        RunsRequest::Fixed(runs) => harness::evaluate(setup, &SeedList::new(seed, runs)),
        RunsRequest::Auto => harness::evaluate_until_stable(setup, seed, EvalOptions::default()),
    }
}

/// Runs `work` on a pool of `threads`, or on the global pool.
pub fn with_threads<T: Send>(
    threads: Option<usize>,
    work: impl FnOnce() -> T + Send,
) -> Result<T, String> {
    match threads {
        Some(threads) => rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map(|pool| pool.install(work))
            .map_err(|error| format!("could not start {threads} threads: {error}")),
        None => Ok(work()),
    }
}

/// Prepares one fight per situation.
pub fn prepare(
    context: &Context,
    party: &PartyFile,
    inputs: &[SituationInput],
) -> Result<Vec<FightSetup>, String> {
    inputs
        .iter()
        .map(|input| {
            FightSetup::new(&context.loaded.data, &context.core, party, &input.situation).map_err(
                |error| {
                    format!(
                        "the fight in {} could not be prepared:\n{error}",
                        input.situation.name
                    )
                },
            )
        })
        .collect()
}

/// Assembles the result file from evaluations already run.
#[allow(clippy::too_many_arguments)]
pub fn result_file(
    context: &Context,
    party: PartyFile,
    inputs: Vec<SituationInput>,
    set: Option<String>,
    seed: u64,
    request: RunsRequest,
    reviewed_only: bool,
    setups: &[FightSetup],
    evaluations: &[Evaluation],
) -> ResultFile {
    let data = &context.loaded.data;
    let pack = PackInfo::of(&context.loaded);
    let situations: Vec<_> = inputs
        .iter()
        .zip(setups)
        .zip(evaluations)
        .map(|((input, setup), evaluation)| results::situation_report(input, evaluation, setup))
        .collect();
    let set_report = set.as_ref().map(|slug| {
        let name = slug
            .parse()
            .ok()
            .and_then(|s| data.situation_sets.get(&s))
            .map(|e| e.value.name.clone())
            .unwrap_or_else(|| slug.clone());
        results::set_report(&name, &situations)
    });
    let setup_refs: Vec<&FightSetup> = setups.iter().collect();
    let notes = Notes::collect(data, pack.clone(), seed, &[&party], evaluations, &setup_refs);
    ResultFile {
        schema_version: results::SCHEMA_VERSION,
        builds: party
            .slots
            .iter()
            .map(|slot| results::build_report(slot, data))
            .collect(),
        inputs: Inputs {
            party,
            situations: inputs,
            set,
            seed,
            runs: request,
            reviewed_only,
            pack,
        },
        situations,
        set: set_report,
        notes,
    }
}

/// Prints a result file as text.
pub fn write_text(out: &mut impl Write, file: &ResultFile, breakdown: bool) -> io::Result<()> {
    report::write_header(
        out,
        &format!("gwsim evaluate: {}", file.inputs.party.name),
        &file.notes,
    )?;
    report::write_builds(out, &file.builds)?;
    for situation in &file.situations {
        report::write_metrics(out, situation)?;
        if breakdown {
            report::write_contributions(out, situation)?;
        }
    }
    if let Some(set) = &file.set {
        report::write_set(out, set)?;
    }
    report::write_footer(out, &file.notes)
}

/// A party: a file, the slug of a party in the data, or up to eight
/// comma-separated skill template codes (the first is the player).
pub fn find_party(data: &DataSet, name: &str) -> Result<PartyFile, String> {
    let path = Path::new(name);
    if path.is_file() {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("could not read {name}: {e}"))?;
        return ron::from_str(&text).map_err(|e| format!("{name} is not a party file: {e}"));
    }
    if let Ok(slug) = name.parse()
        && let Some(party) = data.party(&slug)
    {
        return Ok(party.clone());
    }
    if let Some(party) = party_from_codes(data, name)? {
        return Ok(party);
    }
    Err(format!(
        "{name:?} is not a party file, a party in the data, or a list of skill codes"
    ))
}

/// A party from skill codes, or `None` when the text is not codes at all.
fn party_from_codes(data: &DataSet, text: &str) -> Result<Option<PartyFile>, String> {
    let codes: Vec<&str> = text
        .split(',')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .collect();
    if codes.is_empty() || codes.iter().any(|c| SkillTemplate::decode(c).is_err()) {
        return Ok(None);
    }
    if codes.len() > PARTY_SIZE {
        return Err(format!(
            "a party has at most {PARTY_SIZE} slots; {} codes were given",
            codes.len()
        ));
    }
    let mut slots = Vec::new();
    for (index, code) in codes.iter().enumerate() {
        let template = SkillTemplate::decode(code).map_err(|e| format!("{code}: {e}"))?;
        let (build, _) = Build::from_templates(&template, None, data)
            .map_err(|e| format!("{code}: {e:?}"))?;
        let (name, kind) = if index == 0 {
            ("player".to_owned(), SlotKind::Human)
        } else {
            (format!("hero {index}"), SlotKind::Hero)
        };
        slots.push(PartySlot {
            name,
            kind,
            hero: None,
            build,
            locked: false,
            plan: None,
            disabled_skills: Vec::new(),
            published_code: Some((*code).to_owned()),
        });
    }
    Ok(Some(PartyFile {
        name: format!("{} code(s)", codes.len()),
        notes: String::new(),
        sources: Vec::new(),
        benchmarks: Vec::new(),
        slots,
        tactics: None,
    }))
}

/// The situations to run: one by file or slug, or every entry of a set.
pub fn situation_inputs(
    data: &DataSet,
    situation: Option<&str>,
    set: Option<&str>,
) -> Result<(Vec<SituationInput>, Option<String>), String> {
    if let Some(set) = set {
        let slug = set
            .parse()
            .map_err(|_| format!("{set:?} is not a situation set slug"))?;
        let entry = data
            .situation_sets
            .get(&slug)
            .ok_or_else(|| format!("no situation set {set:?} in the data"))?;
        let inputs = entry
            .value
            .entries
            .iter()
            .map(|e| {
                find_situation(data, e.situation.as_str()).map(|situation| SituationInput {
                    slug: Some(e.situation.to_string()),
                    weight: e.weight,
                    situation,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok((inputs, Some(set.to_owned())));
    }
    let Some(name) = situation else {
        return Err("give --situation or --set".to_owned());
    };
    let situation = find_situation(data, name)?;
    let slug = (!Path::new(name).is_file()).then(|| name.to_owned());
    Ok((
        vec![SituationInput {
            slug,
            weight: 1.0,
            situation,
        }],
        None,
    ))
}

/// A situation by file path, or by slug in the data.
pub fn find_situation(data: &DataSet, name: &str) -> Result<Situation, String> {
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

/// Reads a result file.
pub fn read_result(path: &PathBuf) -> Result<ResultFile, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not JSON: {e}", path.display()))?;
    let version = value["schema_version"].as_u64();
    if version != Some(u64::from(results::SCHEMA_VERSION)) {
        return Err(format!(
            "{} has schema version {version:?}; this gwsim reads version {}",
            path.display(),
            results::SCHEMA_VERSION
        ));
    }
    serde_json::from_value(value)
        .map_err(|e| format!("{} is not a gwsim result file: {e}", path.display()))
}

/// The seed a single run used, for tests.
pub fn first_seed(master: u64) -> RunSeed {
    SeedList::new(master, 1).get(0)
}
