//! The optimiser's relative checks, RC4 and RC5 (T5.9.2, T5.9.4, §17.4).
//!
//! - **RC4**: the PvX player bar ranks highly among random legal player
//!   builds, with the Mesmerway heroes locked, on the M1 set and paired
//!   seeds. It passes when the PvX bar is in the top 10%.
//! - **RC5**: the optimiser does at least as well as the PvX bar: its best
//!   build is no worse, within confidence, in any M1 situation.
//!
//! Builds are ordered as the optimiser ranks them: those meeting the
//! success threshold first, then by how far short they fall, then by the
//! goal (fastest clear). With the M1 chain under 95%, the PvX bar itself
//! may fall short, which is why the order is not "feasible builds only"
//! (fallout F5.12).

use std::collections::BTreeMap;

use gwsim_data::build::SlotKind;
use gwsim_engine::SeedList;
use gwsim_opt::evaluate::{self, Cache, EngineEvaluator, Stages};
use gwsim_opt::genome::{LockMask, PartyGenome, SlotGenome};
use gwsim_opt::objectives::{Goal, Objective, Score, Scoring, ThresholdRule};
use gwsim_opt::operators::SlotContext;
use gwsim_opt::pools::{PoolOptions, SlotPools};
use gwsim_opt::rng::OptRng;
use gwsim_opt::roles::SlotRole;
use gwsim_opt::search::{self, CancelToken, Config, Controls, Problem};

use crate::CheckLevel;
use crate::check::{BENCHMARK, CHECK_SEED, CheckResult, judge};
use crate::compare::Verdict;
use crate::evaluate::{self as eval, Context};
use crate::optimise::party_with;

/// RC4 passes when the PvX bar is in this top share of the sample.
pub const RC4_TOP_SHARE: f64 = 0.10;

/// The seed RC4's random builds are drawn from.
pub const RC4_SAMPLE_SEED: u64 = 0x5243_345f_7361_6d70;

fn sample_size(level: CheckLevel) -> usize {
    match level {
        CheckLevel::Reduced => 200,
        CheckLevel::Full => 1000,
    }
}

fn sample_runs(level: CheckLevel) -> usize {
    match level {
        CheckLevel::Reduced => 16,
        CheckLevel::Full => 32,
    }
}

fn scoring(weights: Vec<f64>) -> Scoring {
    Scoring {
        objectives: vec![Objective::ClearTime, Objective::Deaths],
        goal: Goal::default(),
        threshold: gwsim_opt::objectives::DEFAULT_THRESHOLD,
        rule: ThresholdRule::EverySituation,
        weights,
    }
}

fn order(a: &Score, b: &Score) -> std::cmp::Ordering {
    b.feasible
        .cmp(&a.feasible)
        .then(a.violation.total_cmp(&b.violation))
        .then(a.goal.total_cmp(&b.goal))
}

/// RC4.
pub fn check_rc4(context: &Context, level: CheckLevel) -> Result<CheckResult, String> {
    let data = &context.loaded.data;
    let party = eval::find_party(data, BENCHMARK)?;
    let (inputs, _) = eval::situation_inputs(data, None, Some(crate::check::RC2_SET))?;
    let player = party
        .slots
        .iter()
        .position(|s| s.kind == SlotKind::Human)
        .ok_or("the benchmark has no human slot")?;
    let genome = PartyGenome::from_party(&party, &[(player, LockMask::default())]);
    let SlotGenome::Free(template) = genome.slots[player].clone() else {
        return Err("the player slot is not free".to_owned());
    };
    let mut pools = SlotPools::for_slot(&party.slots[player], data, &PoolOptions::default());
    // The sample keeps the player a Mesmer, as the PvX bar is (T5.9.1).
    pools.primaries = vec![party.slots[player].build.primary];
    let ctx = SlotContext {
        role: SlotRole::infer(&party.slots[player].build, data),
        pools,
    };
    let mut rng = OptRng::new(RC4_SAMPLE_SEED);
    let mut candidates: Vec<(u128, gwsim_data::party::PartyFile)> =
        vec![(genome.hash(), party.clone())];
    for _ in 0..sample_size(level) {
        let mut free = template.clone();
        gwsim_opt::sampler::randomise(&mut free, &ctx, data, &mut rng);
        let mut g = genome.clone();
        g.slots[player] = SlotGenome::Free(free);
        let hash = g.hash();
        if !candidates.iter().any(|(h, _)| *h == hash) {
            candidates.push((hash, g.to_party(&party)));
        }
    }
    let evaluator = EngineEvaluator {
        data,
        core: &context.core,
        situations: inputs.iter().map(|i| i.situation.clone()).collect(),
        title_ranks: BTreeMap::new(),
    };
    let runs = sample_runs(level);
    let seeds = SeedList::new(CHECK_SEED, runs);
    let mut cache = Cache::new(candidates.len() * inputs.len() + 1);
    let refs: Vec<(u128, &gwsim_data::party::PartyFile)> =
        candidates.iter().map(|(h, p)| (*h, p)).collect();
    evaluate::ensure(&evaluator, &mut cache, &seeds, &refs, runs, &|_, _| {})?;
    let rules = scoring(inputs.iter().map(|i| i.weight).collect());
    let scores: Vec<(u128, Score)> = candidates
        .iter()
        .map(|(h, _)| (*h, rules.score(evaluate::stats(&evaluator, &cache, *h))))
        .collect();
    let pvx = &scores[0].1;
    let better = scores[1..]
        .iter()
        .filter(|(_, s)| order(s, pvx).is_lt())
        .count();
    let sample = scores.len() - 1;
    let position = better + 1;
    let share = position as f64 / (sample + 1) as f64;
    let feasible = scores[1..].iter().filter(|(_, s)| s.feasible).count();
    Ok(CheckResult {
        id: "RC4".to_owned(),
        description: "The PvX bar ranks highly among random legal player builds (§17.4)."
            .to_owned(),
        pass: share <= RC4_TOP_SHARE,
        details: vec![
            format!(
                "PvX bar ranks {position} of {} ({:.1}%, needs the top {:.0}%)",
                sample + 1,
                share * 100.0,
                RC4_TOP_SHARE * 100.0
            ),
            format!(
                "PvX bar: goal {:.2}, {}; {feasible} of {sample} random builds meet the threshold",
                pvx.goal,
                if pvx.feasible {
                    "meets the threshold".to_owned()
                } else {
                    format!("short by {:.1} points", pvx.violation * 100.0)
                }
            ),
        ],
        evidence: serde_json::json!({
            "runs": runs,
            "sample": sample,
            "position": position,
            "pvx": pvx,
            "best_random": scores[1..].iter().map(|(_, s)| s).min_by(|a, b| order(a, b)),
        }),
    })
}

/// RC5.
pub fn check_rc5(context: &Context, level: CheckLevel) -> Result<CheckResult, String> {
    let data = &context.loaded.data;
    let party = eval::find_party(data, BENCHMARK)?;
    let (inputs, _) = eval::situation_inputs(data, None, Some(crate::check::RC2_SET))?;
    let player = party
        .slots
        .iter()
        .position(|s| s.kind == SlotKind::Human)
        .ok_or("the benchmark has no human slot")?;
    let free = vec![(player, LockMask::default())];
    let contexts = BTreeMap::from([(
        player,
        SlotContext {
            role: SlotRole::infer(&party.slots[player].build, data),
            pools: SlotPools::for_slot(&party.slots[player], data, &PoolOptions::default()),
        },
    )]);
    let problem = Problem {
        data,
        base: party.clone(),
        free,
        contexts,
        seeds: Vec::new(),
    };
    let evaluator = EngineEvaluator {
        data,
        core: &context.core,
        situations: inputs.iter().map(|i| i.situation.clone()).collect(),
        title_ranks: BTreeMap::new(),
    };
    let mut config = Config::new(
        scoring(inputs.iter().map(|i| i.weight).collect()),
        CHECK_SEED,
    );
    let (population, generations, stages) = match level {
        CheckLevel::Reduced => (
            16,
            3,
            Stages {
                first: 8,
                second: 16,
                frontier: 32,
            },
        ),
        CheckLevel::Full => (32, 10, Stages::default()),
    };
    config.population = population;
    config.max_generations = Some(generations);
    config.stages = stages;
    let outcome = search::optimise(
        &problem,
        &evaluator,
        config,
        Controls {
            should_stop: &|| false,
            cancel: CancelToken::new(),
            on_snapshot: &mut |_| {},
            on_progress: &|_| {},
        },
    )?;
    let best = outcome.ranked.first().ok_or("the search found nothing")?;
    let best_party = party_with(&party, best);

    // Paired comparison at the full run count of the level.
    let runs = crate::check::runs_for(level);
    let seeds = SeedList::new(CHECK_SEED, runs);
    let mut details = vec![format!(
        "best build after {} generations ({} candidates): {}",
        outcome.generations,
        outcome.candidates,
        best.builds
            .iter()
            .map(|b| {
                b.build
                    .skills
                    .iter()
                    .map(|s| {
                        s.and_then(|id| data.skill_by_id(id))
                            .map(|k| k.name.clone())
                            .unwrap_or_else(|| "(empty)".to_owned())
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .collect::<Vec<_>>()
            .join("; ")
    )];
    let mut evidence = Vec::new();
    let mut pass = true;
    for input in &inputs {
        let a = eval::prepare(context, &best_party, std::slice::from_ref(input))?.remove(0);
        let b = eval::prepare(context, &party, std::slice::from_ref(input))?.remove(0);
        let ea = gwsim_engine::harness::evaluate(&a, &seeds);
        let eb = gwsim_engine::harness::evaluate(&b, &seeds);
        let comparison = judge(input, &ea, &eb);
        let worse = comparison.verdict == Verdict::BBetter;
        pass &= !worse;
        details.push(format!(
            "{}: {} (win {:.1}% vs PvX {:.1}%, clear {:.1} s vs {:.1} s)",
            input.situation.name,
            match comparison.verdict {
                Verdict::ABetter => "optimiser better",
                Verdict::BBetter => "optimiser worse",
                Verdict::NotDifferent => "not different",
            },
            ea.win_rate * 100.0,
            eb.win_rate * 100.0,
            ea.clear_time_s.mean,
            eb.clear_time_s.mean
        ));
        evidence.push(serde_json::json!({"situation": input.slug, "comparison": comparison}));
    }
    Ok(CheckResult {
        id: "RC5".to_owned(),
        description: "The optimiser does at least as well as the PvX bar (§17.4).".to_owned(),
        pass,
        details,
        evidence: serde_json::json!({
            "best": best,
            "runs": runs,
            "situations": evidence,
        }),
    })
}
