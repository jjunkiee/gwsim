//! `gwsim compare` — report 7: two parties on the same seeds (T4.9.8).
//!
//! Both parties run every situation on **the same seed list**, so run `i` of
//! each faces the same random numbers (common random numbers, ENG-42). The
//! differences are then judged pair by pair with a paired bootstrap with a
//! fixed seed ([`harness::paired_bootstrap`]), which sees far smaller real
//! differences than comparing two independent intervals would.
//!
//! The same comparison is the relative checks' "A beats B" rule (T4.10.2).

use std::io::{self, Write};

use gwsim_engine::harness::{
    self, BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED, EvalOptions, Evaluation, PairedDifference,
};
use gwsim_engine::result::RunResult;
use gwsim_engine::{FightSetup, SeedList};
use serde::Serialize;

use crate::CompareArgs;
use crate::data::{FAILED, OK};
use crate::evaluate::{self, DEFAULT_SEED, Failure};
use crate::report;
use crate::results::{Notes, PackInfo, SituationInput};

/// Which direction of a metric is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Better {
    Higher,
    Lower,
}

/// One metric of both parties, and the paired difference A − B.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetricDiff {
    pub metric: String,
    pub a: f64,
    pub b: f64,
    pub difference: PairedDifference,
    pub better: Better,
}

impl MetricDiff {
    /// Whether A is significantly better on this metric.
    pub fn a_better(&self) -> bool {
        match self.better {
            Better::Higher => self.difference.above_zero(),
            Better::Lower => self.difference.below_zero(),
        }
    }

    /// Whether B is significantly better on this metric.
    pub fn b_better(&self) -> bool {
        match self.better {
            Better::Higher => self.difference.below_zero(),
            Better::Lower => self.difference.above_zero(),
        }
    }
}

/// Who came out ahead (the T4.10.2 rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Verdict {
    ABetter,
    BBetter,
    NotDifferent,
}

/// Both parties in one situation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SituationComparison {
    pub name: String,
    pub slug: Option<String>,
    pub runs: usize,
    pub metrics: Vec<MetricDiff>,
    pub verdict: Verdict,
}

/// Runs both setups on the same seeds: `runs` each, or each until stable
/// with the shorter extended to the longer.
pub fn paired_runs(
    a: &FightSetup,
    b: &FightSetup,
    runs: Option<usize>,
    seed: u64,
) -> (Evaluation, Evaluation) {
    match runs {
        Some(n) => {
            let seeds = SeedList::new(seed, n);
            (harness::evaluate(a, &seeds), harness::evaluate(b, &seeds))
        }
        None => {
            let first = harness::evaluate_until_stable(a, seed, EvalOptions::default());
            let second = harness::evaluate_until_stable(b, seed, EvalOptions::default());
            let n = first.runs.max(second.runs);
            (
                harness::extend_to(a, first, seed, n),
                harness::extend_to(b, second, seed, n),
            )
        }
    }
}

fn paired(
    a: &[RunResult],
    b: &[RunResult],
    f: impl Fn(&RunResult) -> Option<f64>,
) -> (Vec<f64>, Vec<f64>) {
    a.iter()
        .zip(b)
        .filter_map(|(x, y)| Some((f(x)?, f(y)?)))
        .unzip()
}

/// The metrics of two paired evaluations.
pub fn differences(a: &Evaluation, b: &Evaluation) -> Vec<MetricDiff> {
    type Getter = fn(&RunResult) -> Option<f64>;
    let metrics: [(&str, Better, Getter); 5] = [
        ("win rate", Better::Higher, |r| Some(if r.won() { 1.0 } else { 0.0 })),
        ("clear time (s)", Better::Lower, |r| {
            r.clear_time_ms.map(|ms| f64::from(ms) / 1000.0)
        }),
        ("deaths", Better::Lower, |r| Some(f64::from(r.deaths))),
        ("damage taken", Better::Lower, |r| Some(r.damage_taken as f64)),
        ("DP at end (%)", Better::Lower, |r| Some(f64::from(r.dp_end))),
    ];
    metrics
        .iter()
        .map(|(name, better, get)| {
            let (xs, ys) = paired(&a.results, &b.results, get);
            let mean = |v: &[f64]| {
                if v.is_empty() {
                    0.0
                } else {
                    v.iter().sum::<f64>() / v.len() as f64
                }
            };
            MetricDiff {
                metric: (*name).to_owned(),
                a: mean(&xs),
                b: mean(&ys),
                difference: harness::paired_bootstrap(&xs, &ys, BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED),
                better: *better,
            }
        })
        .collect()
}

/// "A beats B" (T4.10.2): the win-rate difference's interval is above zero;
/// or the win rates are equal within the interval and the clear-time
/// difference's interval is below zero.
pub fn verdict(metrics: &[MetricDiff]) -> Verdict {
    let win = &metrics[0];
    let clear = &metrics[1];
    if win.a_better() {
        return Verdict::ABetter;
    }
    if win.b_better() {
        return Verdict::BBetter;
    }
    if clear.a_better() {
        Verdict::ABetter
    } else if clear.b_better() {
        Verdict::BBetter
    } else {
        Verdict::NotDifferent
    }
}

/// Compares two setups in one situation.
pub fn compare_setups(
    input: &SituationInput,
    a: &FightSetup,
    b: &FightSetup,
    runs: Option<usize>,
    seed: u64,
) -> (SituationComparison, Evaluation, Evaluation) {
    let (ea, eb) = paired_runs(a, b, runs, seed);
    let metrics = differences(&ea, &eb);
    let comparison = SituationComparison {
        name: input.situation.name.clone(),
        slug: input.slug.clone(),
        runs: ea.runs,
        verdict: verdict(&metrics),
        metrics,
    };
    (comparison, ea, eb)
}

/// Runs `gwsim compare`.
pub fn run(args: &CompareArgs, out: &mut impl Write) -> io::Result<i32> {
    match compare(args, out) {
        Ok(code) => Ok(code),
        Err(Failure::Io(error)) => Err(error),
        Err(Failure::Message(message)) => {
            writeln!(out, "{message}")?;
            Ok(FAILED)
        }
    }
}

fn compare(args: &CompareArgs, out: &mut impl Write) -> Result<i32, Failure> {
    let context = evaluate::load_context(args.data_dir.as_deref())?;
    let data = &context.loaded.data;
    let party_a = evaluate::find_party(data, &args.party_a)?;
    let party_b = evaluate::find_party(data, &args.party_b)?;
    let is_set = args
        .situations
        .parse()
        .ok()
        .is_some_and(|slug| data.situation_sets.contains_key(&slug));
    let (inputs, _) = if is_set {
        evaluate::situation_inputs(data, None, Some(&args.situations))?
    } else {
        evaluate::situation_inputs(data, Some(&args.situations), None)?
    };
    let setups_a = evaluate::prepare(&context, &party_a, &inputs)?;
    let setups_b = evaluate::prepare(&context, &party_b, &inputs)?;
    let seed = args.seed.unwrap_or(DEFAULT_SEED);

    let outcomes = evaluate::with_threads(args.threads, || {
        inputs
            .iter()
            .zip(setups_a.iter().zip(&setups_b))
            .map(|(input, (a, b))| compare_setups(input, a, b, args.runs, seed))
            .collect::<Vec<_>>()
    })?;

    let evaluations: Vec<&Evaluation> = outcomes.iter().flat_map(|(_, a, b)| [a, b]).collect();
    let setups: Vec<&FightSetup> = setups_a.iter().chain(&setups_b).collect();
    let mut notes = Notes::collect(
        data,
        PackInfo::of(&context.loaded),
        seed,
        &[&party_a, &party_b],
        evaluations,
        &setups,
    );
    // Both parties share each situation's seeds, so count runs once.
    notes.runs = outcomes.iter().map(|(c, _, _)| c.runs).collect();
    let comparisons: Vec<SituationComparison> =
        outcomes.into_iter().map(|(c, _, _)| c).collect();

    if args.json {
        let value = serde_json::json!({
            "schema_version": crate::results::SCHEMA_VERSION,
            "a": party_a.name,
            "b": party_b.name,
            "seed": seed,
            "bootstrap": {"resamples": BOOTSTRAP_RESAMPLES, "seed": BOOTSTRAP_SEED},
            "situations": comparisons,
            "notes": notes,
        });
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).map_err(io::Error::other)?
        )?;
        return Ok(OK);
    }

    report::write_header(
        out,
        &format!("gwsim compare: A = {}, B = {}", party_a.name, party_b.name),
        &notes,
    )?;
    for comparison in &comparisons {
        write_comparison(out, comparison, &party_a.name, &party_b.name)?;
    }
    report::write_footer(out, &notes)?;
    Ok(OK)
}

/// One situation's comparison as text.
pub fn write_comparison(
    out: &mut impl Write,
    comparison: &SituationComparison,
    a: &str,
    b: &str,
) -> io::Result<()> {
    let label = comparison
        .slug
        .as_deref()
        .map(|slug| format!("{} [{slug}]", comparison.name))
        .unwrap_or_else(|| comparison.name.clone());
    writeln!(out, "{label}: {} paired runs", comparison.runs)?;
    writeln!(
        out,
        "  {:<16} {:>10} {:>10} {:>10}   95% paired interval",
        "metric", "A", "B", "A - B"
    )?;
    for m in &comparison.metrics {
        let scale = if m.metric == "win rate" { 100.0 } else { 1.0 };
        let mark = if m.a_better() {
            "  A better"
        } else if m.b_better() {
            "  B better"
        } else {
            ""
        };
        writeln!(
            out,
            "  {:<16} {:>10.2} {:>10.2} {:>+10.2}   {:+.2} to {:+.2}{mark}",
            if scale > 1.0 { "win rate (%)" } else { &m.metric },
            m.a * scale,
            m.b * scale,
            m.difference.mean * scale,
            m.difference.ci.low * scale,
            m.difference.ci.high * scale,
        )?;
    }
    let verdict = match comparison.verdict {
        Verdict::ABetter => format!("A ({a}) is better"),
        Verdict::BBetter => format!("B ({b}) is better"),
        Verdict::NotDifferent => "no significant difference".to_owned(),
    };
    writeln!(out, "  verdict: {verdict}")?;
    writeln!(out)
}
